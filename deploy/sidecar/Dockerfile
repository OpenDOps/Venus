# Venus snapshotter (Rust + y-octo hydrate + Node Path B CLI). MIT OR Apache-2.0.
# Not OctoBase. Do not COPY deploy/octobase into this image.
# Health is GET / → venus-sidecar (process boots). No git init/commit in step-worker.
# Runtime is node:22-bookworm so the binary can spawn from-pinned-cli.js.
# Hub image must not use this binary as its entrypoint.

FROM rust:1.98.1-bookworm AS rust-build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/venus-hub crates/venus-hub
COPY crates/venus-sidecar crates/venus-sidecar
RUN cargo build --release -p venus-sidecar

FROM node:22-bookworm AS node-deps
WORKDIR /src
RUN corepack enable
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY apps/web/package.json apps/web/
RUN pnpm install --frozen-lockfile
COPY apps/web apps/web

FROM node:22-bookworm
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 65532 venus \
    && useradd --system --uid 65532 --gid venus --create-home \
        --home-dir /home/venus --shell /usr/sbin/nologin venus

COPY --from=rust-build --chown=venus:venus /src/target/release/venus-sidecar /usr/local/bin/venus-sidecar
COPY --from=node-deps --chown=venus:venus /src /opt/venus

ENV SIDECAR_LISTEN=0.0.0.0:3002
ENV CONVERT_CWD=/opt/venus/apps/web
ENV CONVERT_CLI=/opt/venus/apps/web/src/host/mdgate/from-pinned-cli.js
ENV VENUS_BLOB_ORIGIN=http://hub:3000
ENV HOME=/home/venus

EXPOSE 3002
USER venus
WORKDIR /opt/venus
CMD ["venus-sidecar"]
