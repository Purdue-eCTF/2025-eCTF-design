FROM --platform=linux/amd64 nixos/nix:2.19.3

ENV NIX_CONFIG="filter-syscalls = false"
RUN nix-channel --update

COPY . ectf

