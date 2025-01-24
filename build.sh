#!/bin/sh

rm -rf build_out && mkdir -p build_out
trap "docker compose down" EXIT
docker compose up --build
