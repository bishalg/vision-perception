# syntax=docker/dockerfile:1
# owner: WP-B.1 (DEP-1)
#
# One image for the ASD backend: `asd-backend serve | worker | migrate` and `asd-cli`
# (compose `seed`). Build context is `rust/`:
#
#   docker build -f rust/Dockerfile -t asd-backend rust
#
# Stages: cargo-chef planner -> dependency cook (cached until Cargo.lock changes) ->
# release build -> distroless runtime (glibc + libgcc, no shell, non-root).
# sqlx query macros compile against the committed `.sqlx/` data (SQLX_OFFLINE=true);
# no database is needed at build time.

ARG RUST_IMAGE=rust:1-bookworm

FROM ${RUST_IMAGE} AS chef
RUN cargo install cargo-chef --locked
WORKDIR /build

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ENV SQLX_OFFLINE=true \
    CARGO_TERM_COLOR=never
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked --recipe-path recipe.json -p asd-backend -p asd-cli
COPY . .
RUN cargo build --release --locked -p asd-backend -p asd-cli

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder /build/target/release/asd-backend /usr/local/bin/asd-backend
COPY --from=builder /build/target/release/asd-cli /usr/local/bin/asd-cli
ENV ASD_BIND_ADDR=0.0.0.0:8000
EXPOSE 8000
USER nonroot:nonroot
ENTRYPOINT ["/usr/local/bin/asd-backend"]
CMD ["serve"]
