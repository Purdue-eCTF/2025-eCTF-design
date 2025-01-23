# FROM ubuntu:24.04
FROM rust:1.84.0-bullseye

LABEL version="0.1"
LABEL description="Purdue 2025 eCTF"

ARG DEBIAN_FRONTEND=noninteractive
WORKDIR /root

# Install Requisite Packages
# do this first because it takes the longest
RUN apt-get update && apt-get upgrade -y && apt-get install -y \
    gdb \
    gdb-multiarch \
    gcc-arm-none-eabi \
    binutils-arm-none-eabi \
    make \
    python3.10 \
    git \
    wget \
    libtool \
    texinfo \
    unzip    

RUN cargo install --force cargo-make

# install toolchain in dockerfile so it does not need to be installed on every compilation
RUN rustup toolchain install --component rustfmt --component rustc-dev --target thumbv7em-none-eabihf --profile minimal nightly-2024-02-19

WORKDIR /decoder

ENTRYPOINT ["bash", "-c", "cd decoder/ && cargo make -e BUILD_TYPE=release -e CARGO_TARGET_DIR=${CURDIR}/target decoder --verbose"]

# Sample run command:
# docker run --rm -v ./build_out:/out -v ./:/decoder -v ./secrets:/secrets -e DECODER_ID=0xdeadbeef decoder
