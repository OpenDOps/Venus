# syntax=docker/dockerfile:1
# OctoBase keck (AGPL-3.0). Pin must match docs/design/api-map.md.
FROM rust:1.85-bookworm AS builder

ARG OCTOBASE_SHA=276e0e94719a652483119c5fea16be13293ee21c

RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates pkg-config cmake clang libclang-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
RUN git init \
    && git remote add origin https://github.com/toeverything/OctoBase.git \
    && git fetch --depth 1 origin "${OCTOBASE_SHA}" \
    && git checkout --force FETCH_HEAD

# Y.Text marks: Content::Format → Value::Format (stock is unimplemented!()).
# Overlay, not git apply: unified diffs against this pin failed on hunk context.
COPY patches/value.rs /src/libs/jwst-codec/src/doc/types/value.rs
COPY patches/publisher.rs /src/libs/jwst-codec/src/doc/publisher.rs

ENV CARGO_TERM_COLOR=always \
    CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

# Cache registry + target across rebuilds; copy the binary out of the cache mount.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build -p keck --release \
    && cp /src/target/release/keck /keck

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --no-create-home --uid 10001 keck

COPY --from=builder /keck /usr/local/bin/keck

EXPOSE 3000
USER keck
ENTRYPOINT ["keck"]
