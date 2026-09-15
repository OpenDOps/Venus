//! JSON envelope for HTTP GET / collab errors. Same `error` object as
//! `venus.rpc.v1.Error`. gRPC Status.details uses the proto. Product export
//! is gRPC; GET `/export` is advertisement only.
//! SPDX-License-Identifier: MIT OR Apache-2.0

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use crate::{CATALOG_DOC_ID, PAGE_DOC_ID};

pub const CODE_INVALID_WORKSPACE: &str = "invalid_workspace";
pub const CODE_INVALID_DOC: &str = "invalid_doc";
pub const CODE_EXPORT_HTTP_DISABLED: &str = "export_http_disabled";
pub const CODE_LEASE_HELD: &str = "lease_held";
pub const CODE_STORE_FAILED: &str = "store_failed";

pub const GRPC_SERVICE: &str = "venus.hub.v1.Hub";
pub const GRPC_LISTEN_ENV: &str = "HUB_GRPC_LISTEN";
pub const GRPC_LISTEN_DEFAULT: &str = "0.0.0.0:3100";

/// Root `error` object. Callers treat any present `error` as failure.
pub fn error_body(code: &str, message: &str) -> Value {
    json!({ "code": code, "message": message })
}

pub fn error_response(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({ "error": error_body(code, message) }))).into_response()
}

pub fn error_lease_held(workspace_id: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": {
                "code": CODE_LEASE_HELD,
                "message": "workspace owned by another hub",
                "workspace_id": workspace_id,
            }
        })),
    )
        .into_response()
}

pub fn error_store_failed(workspace_id: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": {
                "code": CODE_STORE_FAILED,
                "message": "hub store failed",
                "workspace_id": workspace_id,
            }
        })),
    )
        .into_response()
}

pub fn advertisement_response(advertisement: Value) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "advertisement": advertisement })),
    )
        .into_response()
}

pub fn error_with_advertisement(
    status: StatusCode,
    code: &str,
    message: &str,
    advertisement: Value,
) -> Response {
    (
        status,
        Json(json!({
            "error": error_body(code, message),
            "advertisement": advertisement,
        })),
    )
        .into_response()
}

/// Cheap discovery: home + catalog constants, gRPC bind, `?doc=` / `doc_id` rules.
/// Does not encode Yjs and does not hit SQL.
pub fn export_advertisement(workspace_id: &str) -> Value {
    json!({
        "kind": "doc_export",
        "workspace_id": workspace_id,
        "http_export": false,
        "grpc": {
            "service": GRPC_SERVICE,
            "methods": ["ListDocs", "ExportDoc"],
            "listen_env": GRPC_LISTEN_ENV,
            "listen_default": GRPC_LISTEN_DEFAULT,
        },
        "docs": [
            {
                "sql_id": PAGE_DOC_ID,
                "guid": "doc:home",
                "role": "home",
            },
            {
                "sql_id": CATALOG_DOC_ID,
                "guid": "venus:catalog",
                "role": "catalog",
            },
        ],
        "doc": {
            "param": "doc_id",
            "format": "sql uuid",
            "omit": "home",
            "ws_query": "?doc=<sql uuid> on GET /collaboration/:workspace_id (omit = home; guid is 400)",
            "grpc": "ExportDocRequest.doc_id; omit = home. Guid is invalid_doc.",
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_omits_error_key() {
        let v = json!({ "advertisement": export_advertisement(crate::DEFAULT_WORKSPACE_ID) });
        assert!(v.get("error").is_none());
        assert_eq!(v["advertisement"]["http_export"], false);
        assert_eq!(v["advertisement"]["docs"][0]["role"], "home");
        assert_eq!(v["advertisement"]["docs"][1]["role"], "catalog");
        assert_eq!(v["advertisement"]["grpc"]["service"], GRPC_SERVICE);
    }

    #[test]
    fn clients_check_root_error() {
        let v = json!({
            "error": error_body(CODE_EXPORT_HTTP_DISABLED, "no GET yjs"),
            "advertisement": { "kind": "doc_export" },
        });
        assert!(v.get("error").is_some());
        assert_eq!(v["error"]["code"], CODE_EXPORT_HTTP_DISABLED);
    }
}
