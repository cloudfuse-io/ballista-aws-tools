# syntax=docker/dockerfile:experimental

# This docker file contains different build targets of the application
# Each target is represented by a stage

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

# use BuildKit experimental cache mount to speed up builds
RUN --mount=type=cache,target=./target \
  --mount=type=cache,target=/usr/local/cargo/git \
  --mount=type=cache,target=/usr/local/cargo/registry \
  cargo build --bin ${BIN_NAME} --release --target=x86_64-unknown-linux-musl && \
  cp ./target/x86_64-unknown-linux-musl/release/${BIN_NAME} ./exec-static


## RUNTIME STAGE ##
# A runtime container

FROM scratch as runtime-stage
ARG PORT
EXPOSE ${PORT}
COPY --from=build-stage /buildspace/exec-static /app
COPY --from=build-stage /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
COPY --from=build-stage /tmp /tmp

ENTRYPOINT ["./app"]