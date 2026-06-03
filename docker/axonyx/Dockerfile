FROM rust:1.95-slim

ARG CARGO_AXONYX_VERSION=0.1.42
ARG CREATE_AXONYX_VERSION=0.1.24
ARG AXONYX_RUNTIME_VERSION=0.1.15

LABEL org.opencontainers.image.title="Axonyx"
LABEL org.opencontainers.image.description="Rust-first Axonyx framework demo image with a generated docs starter"
LABEL org.opencontainers.image.source="https://github.com/vladanPro/axonyx-framework"
LABEL org.opencontainers.image.version="${CARGO_AXONYX_VERSION}"
LABEL org.opencontainers.image.licenses="MIT"

ENV AXONYX_HOST=0.0.0.0
ENV AXONYX_PORT=3000
ENV CARGO_AXONYX_VERSION=${CARGO_AXONYX_VERSION}
ENV CREATE_AXONYX_VERSION=${CREATE_AXONYX_VERSION}
ENV AXONYX_RUNTIME_VERSION=${AXONYX_RUNTIME_VERSION}

WORKDIR /opt/axonyx

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install create-axonyx --version "${CREATE_AXONYX_VERSION}" --force \
    && cargo install cargo-axonyx --version "${CARGO_AXONYX_VERSION}" --force

RUN create-axonyx app --yes --template docs --runtime-source registry --runtime-version "${AXONYX_RUNTIME_VERSION}" \
    && cd app \
    && cargo ax upgrade --skip-cli \
    && cargo ax build --clean

WORKDIR /opt/axonyx/app

COPY docker/axonyx/entrypoint.sh /usr/local/bin/axonyx-demo
RUN chmod +x /usr/local/bin/axonyx-demo

EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -fsS "http://127.0.0.1:${PORT:-${AXONYX_PORT}}/__axonyx/health" || exit 1

CMD ["axonyx-demo"]
