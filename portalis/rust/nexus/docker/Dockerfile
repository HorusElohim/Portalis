FROM rust:1.97.1-bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build --locked --release -p portalis-nexus-server

FROM debian:bookworm-slim AS runtime

RUN apt-get update \
    && apt-get install --yes --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --no-create-home nexus

COPY --from=builder /workspace/target/release/portalis-nexus-server /usr/local/bin/portalis-nexus-server

USER 10001:10001
ENV PORTALIS_NEXUS_LISTEN_ADDR=0.0.0.0:8080
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/portalis-nexus-server"]
