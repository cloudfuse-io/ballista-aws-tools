# syntax=docker/dockerfile:experimental

# This docker file contains different build targets of the application
# Each target is represented by a stage
# use BuildKit experimental cache mount to speed up builds (--mount=type=cache,target=xxx)

## BUILD STAGE ##
# Create a static binary (with musl) for one of the binary targets.
FROM rust:1.53.0-buster as build-stage
ARG BIN_NAME
# install environment
RUN apt-get update
RUN apt-get install musl-tools zip -y
WORKDIR /buildspace
RUN rustup target add x86_64-unknown-linux-musl && rustup component add rustfmt 
COPY ./rust .
# copy tpch from vendor for local integ tests (docker compose)
RUN --mount=type=cache,target=./target \
  --mount=type=cache,target=/usr/local/cargo/git/db/ \
  --mount=type=cache,target=/usr/local/cargo/registry/index/ \
  --mount=type=cache,target=/usr/local/cargo/registry/cache/ \
  rm -rf vendor/ && \
  cargo vendor vendor/ && \
  cp vendor/arrow-benchmarks/src/bin/tpch.rs src/bin/
# build on musl platform and save 
RUN --mount=type=cache,target=./target \
  --mount=type=cache,target=/usr/local/cargo/git/db/ \
  --mount=type=cache,target=/usr/local/cargo/registry/index/ \
  --mount=type=cache,target=/usr/local/cargo/registry/cache/ \
  cargo build --bin ${BIN_NAME} --release --target=x86_64-unknown-linux-musl && \
  cp ./target/x86_64-unknown-linux-musl/release/${BIN_NAME} ./exec-static

## LAMBDA PACKAGE STAGE ##
# Create a zip archive to be deployed to AWS Lambda
FROM build-stage as package-stage
ARG BIN_NAME
# the exec name inside a lambda archive should be `bootstrap`
RUN cp ./exec-static ./bootstrap
RUN zip ${BIN_NAME}.zip bootstrap

## RUNTIME STAGE ##
# A runtime container
FROM scratch as runtime-stage
ARG PORT
EXPOSE ${PORT}
COPY --from=build-stage /buildspace/exec-static /app
COPY --from=build-stage /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build-stage /tmp /tmp
ENTRYPOINT ["./app"]

## BENCHMARK STAGE ##
# A runtime container for executing tpch queries
FROM scratch as tpch-stage
COPY --from=build-stage /buildspace/exec-static /app
COPY --from=build-stage /buildspace/src/tpch_queries /queries
COPY --from=build-stage /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build-stage /tmp /tmp
ENTRYPOINT ["./app"]

## EXPORT STAGE ##
# Isolate the archive so it can be exported with `docker build -o`
FROM scratch as export-stage
ARG BIN_NAME
COPY --from=package-stage /buildspace/${BIN_NAME}.zip /