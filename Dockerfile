FROM debian:bookworm-slim as builder

RUN apt-get update && apt-get install -y \
    curl \
    ca-certificates \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable \
    && rustup default stable

WORKDIR /app

COPY . .

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN useradd -m -u 1000 dns-proxy
COPY --from=builder /app/target/release/dns-proxy /usr/local/bin/dns-proxy

COPY config.toml.example /app/config.toml.example
WORKDIR /app
RUN chown -R dns-proxy:dns-proxy /app
USER dns-proxy
# Expose ports: DoT: 853, DoH: 443, DoQ: 853, DoH3: 443, Healthcheck: 8080
EXPOSE 853 443 8080
ENTRYPOINT ["/usr/local/bin/dns-proxy"]

