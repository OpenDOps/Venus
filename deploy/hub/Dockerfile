# Venus hub (Rust + y-octo). Product collab front. MIT OR Apache-2.0.
# Not OctoBase. Do not COPY deploy/octobase into this image.
# Runtime is debian:bookworm-slim (not the toolchain image) as USER venus (S5).
# Do not bake POSTGRES_PASSWORD / DATABASE_URL into the image (Compose injects them).

FROM rust:1.90-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/venus-hub crates/venus-hub
RUN cargo build --release -p venus-hub

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --system --gid 65532 venus \
    && useradd --system --uid 65532 --gid venus --no-create-home \
        --home-dir /nonexistent --shell /usr/sbin/nologin venus

COPY --from=build --chown=venus:venus /src/target/release/venus-hub /usr/local/bin/venus-hub

ENV POSTGRES_HOST=postgres
ENV POSTGRES_PORT=5432
ENV POSTGRES_DB=venus
ENV HUB_LISTEN=0.0.0.0:3000

EXPOSE 3000
USER venus
CMD ["venus-hub"]
