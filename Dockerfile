ARG RUST_VERSION=1.81
ARG DEBIAN_VERSION=bookworm-slim

FROM rust:${RUST_VERSION} AS rust-chef
RUN cargo install cargo-chef

FROM rust-chef AS planner

WORKDIR /usr/src/app
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM rust-chef AS builder

WORKDIR /usr/src/app
COPY --from=planner /usr/src/app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY . .
RUN cargo build --release

FROM debian:${DEBIAN_VERSION} AS runtime

RUN apt-get update -y && \
    apt-get install --no-install-recommends -y iproute2 iptables ca-certificates && \
    apt-get clean && rm -f /var/lib/apt/lists/*_*

COPY --from=docker.io/tailscale/tailscale:stable /usr/local/bin/tailscaled /usr/bin/tailscaled
COPY --from=docker.io/tailscale/tailscale:stable /usr/local/bin/tailscale /usr/bin/tailscale
COPY --from=builder /usr/src/app/target/release/tailscale-hello /usr/bin
COPY --from=builder /usr/src/app/start.sh /

CMD ["/start.sh"]
