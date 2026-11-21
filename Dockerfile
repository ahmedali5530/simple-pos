# Till pages and API in one image. Postgres stays outside the container.
# Caddy or Nginx on the VPS proxies the domain to 127.0.0.1:8787.

FROM node:22-bookworm-slim AS ui
WORKDIR /src
COPY package.json package-lock.json ./
RUN npm ci
COPY . .
RUN npm run build

FROM rust:1-bookworm AS api
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libudev-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates crates
COPY config.toml config.toml
COPY --from=ui /src/dist ./dist
RUN cargo build -p shop-api --release --features web

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates libudev1 cups-client \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=api /src/target/release/shop-api /app/shop-api
COPY config.toml /app/config.toml
ENV API_ADDR=0.0.0.0:8787 \
    IMAGE_DIR=/data/item-images
EXPOSE 8787
VOLUME /data/item-images
CMD ["/app/shop-api"]
