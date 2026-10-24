# The `apix` command line in a small image, for CI systems that run containers:
#
#   docker run --rm -v "$PWD:/work" -e APIX_VAR_TOKEN ghcr.io/11r5/apix run api --env staging
#
# Build it yourself with `docker build -t apix .` from the repository root.

FROM rust:1-slim-bookworm AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY . .
RUN cargo build --release --locked -p apix-cli

FROM debian:bookworm-slim
# Root certificates for HTTPS; nothing else is needed at run time.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/apix /usr/local/bin/apix
WORKDIR /work
ENTRYPOINT ["apix"]
CMD ["--help"]
