FROM rust:1.95-slim

ARG CARGO_AXONYX_VERSION=0.1.42
ARG CREATE_AXONYX_VERSION=0.1.23
ARG AXONYX_RUNTIME_VERSION=0.1.15

ENV AXONYX_HOST=0.0.0.0
ENV AXONYX_PORT=3000

WORKDIR /opt/axonyx

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install create-axonyx --version "${CREATE_AXONYX_VERSION}" --force \
    && cargo install cargo-axonyx --version "${CARGO_AXONYX_VERSION}" --force

RUN create-axonyx app --yes --template docs --runtime-source registry --runtime-version "${AXONYX_RUNTIME_VERSION}" \
    && cd app \
    && cargo ax upgrade --skip-cli \
    && cargo ax build --clean

WORKDIR /opt/axonyx/app

EXPOSE 3000

CMD ["sh", "-c", "cargo ax run start --host ${AXONYX_HOST} --port ${PORT:-${AXONYX_PORT}}"]
