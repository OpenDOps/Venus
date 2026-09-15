//! Internal gRPC `venus.hub.v1.Hub` (`ExportDoc`, `ListDocs`).
//! SPDX-License-Identifier: MIT OR Apache-2.0

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use prost::Message;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tonic::{Code, Request, Response, Status};

use crate::db;
use crate::http::{bind_doc_id, workspace_id_ok};
use crate::room::Hub;
use crate::rpc::{
    CODE_INVALID_DOC, CODE_INVALID_WORKSPACE, CODE_STORE_FAILED, GRPC_LISTEN_DEFAULT,
    GRPC_LISTEN_ENV, GRPC_SERVICE,
};
use crate::{CATALOG_DOC_ID, PAGE_DOC_ID};

pub mod pb {
    pub mod venus {
        pub mod hub {
            pub mod v1 {
                tonic::include_proto!("venus.hub.v1");
            }
        }
        pub mod rpc {
            pub mod v1 {
                tonic::include_proto!("venus.rpc.v1");
            }
        }
    }
}

use pb::venus::hub::v1::hub_server::{Hub as HubRpc, HubServer};
use pb::venus::hub::v1::{ExportDocRequest, ExportDocResponse, ListDocsRequest, ListDocsResponse};
use pb::venus::rpc::v1::{Advertisement, DocParam, DocRef, Error as RpcError, GrpcBind};

pub struct HubGrpc {
    hub: Arc<Hub>,
}

impl HubGrpc {
    pub fn new(hub: Arc<Hub>) -> Self {
        Self { hub }
    }
}

#[tonic::async_trait]
impl HubRpc for HubGrpc {
    async fn export_doc(
        &self,
        request: Request<ExportDocRequest>,
    ) -> Result<Response<ExportDocResponse>, Status> {
        let req = request.into_inner();
        let workspace_id = parse_workspace(&req.workspace_id)?;
        let doc_id = parse_export_doc_id(req.doc_id.as_deref())?;
        match self.hub.live_export_doc(&workspace_id, &doc_id).await {
            Ok(bin) => Ok(Response::new(ExportDocResponse {
                yjs_update_v1: bin,
                doc_id,
            })),
            Err(e) => Err(status_store(&workspace_id, e)),
        }
    }

    async fn list_docs(
        &self,
        request: Request<ListDocsRequest>,
    ) -> Result<Response<ListDocsResponse>, Status> {
        let req = request.into_inner();
        let workspace_id = parse_workspace(&req.workspace_id)?;
        let mut docs = vec![home_ref(), catalog_ref()];
        match db::list_created_pages(&self.hub.pool, &workspace_id).await {
            Ok(rows) => {
                for (uuid, guid, _name, _path) in rows {
                    docs.push(DocRef {
                        sql_id: uuid,
                        guid,
                        role: "page".into(),
                    });
                }
            }
            Err(e) => return Err(status_store(&workspace_id, e)),
        }
        Ok(Response::new(ListDocsResponse {
            docs,
            advertisement: Some(advertisement_msg(&workspace_id)),
        }))
    }
}

fn parse_workspace(id: &str) -> Result<String, Status> {
    if !workspace_id_ok(id) {
        return Err(status_error(
            Code::InvalidArgument,
            CODE_INVALID_WORKSPACE,
            "invalid workspace_id",
            id,
        ));
    }
    Ok(id.to_ascii_lowercase())
}

fn parse_export_doc_id(raw: Option<&str>) -> Result<String, Status> {
    bind_doc_id(raw)
        .map_err(|_| status_error(Code::InvalidArgument, CODE_INVALID_DOC, "invalid doc", ""))
}

fn status_store(workspace_id: &str, e: anyhow::Error) -> Status {
    tracing::error!(workspace_id = %workspace_id, error = %e, "grpc store");
    status_error(
        Code::Internal,
        CODE_STORE_FAILED,
        "hub store failed",
        workspace_id,
    )
}

fn status_error(code: Code, err_code: &str, message: &str, workspace_id: &str) -> Status {
    let details = RpcError {
        code: err_code.into(),
        message: message.into(),
        workspace_id: workspace_id.into(),
    }
    .encode_to_vec();
    Status::with_details(code, message, details.into())
}

fn home_ref() -> DocRef {
    DocRef {
        sql_id: PAGE_DOC_ID.into(),
        guid: "doc:home".into(),
        role: "home".into(),
    }
}

fn catalog_ref() -> DocRef {
    DocRef {
        sql_id: CATALOG_DOC_ID.into(),
        guid: "venus:catalog".into(),
        role: "catalog".into(),
    }
}

fn advertisement_msg(workspace_id: &str) -> Advertisement {
    Advertisement {
        kind: "doc_export".into(),
        workspace_id: workspace_id.into(),
        http_export: false,
        grpc: Some(GrpcBind {
            service: GRPC_SERVICE.into(),
            methods: vec!["ListDocs".into(), "ExportDoc".into()],
            listen_env: GRPC_LISTEN_ENV.into(),
            listen_default: GRPC_LISTEN_DEFAULT.into(),
        }),
        docs: vec![home_ref(), catalog_ref()],
        doc: Some(DocParam {
            param: "doc_id".into(),
            format: "sql uuid".into(),
            omit: "home".into(),
            ws_query:
                "?doc=<sql uuid> on GET /collaboration/:workspace_id (omit = home; guid is 400)"
                    .into(),
            grpc: "ExportDocRequest.doc_id; omit = home. Guid is invalid_doc.".into(),
        }),
    }
}

/// Bind and serve until the listener drops. Tests use [`serve_listener`].
pub async fn serve(hub: Arc<Hub>, addr: SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    serve_listener(hub, listener).await
}

pub async fn serve_listener(hub: Arc<Hub>, listener: TcpListener) -> Result<()> {
    let incoming = TcpListenerStream::new(listener);
    Server::builder()
        .add_service(HubServer::new(HubGrpc::new(hub)))
        .serve_with_incoming(incoming)
        .await?;
    Ok(())
}
