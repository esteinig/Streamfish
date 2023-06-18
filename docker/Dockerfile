FROM rust:1.69-bookworm

ENV CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse

WORKDIR /usr/src/reefsquid

RUN apt update && apt-get -y install make cmake libprotobuf-dev protobuf-compiler 
RUN cargo install cargo-watch

COPY ../ ./

