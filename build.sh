#!/bin/sh

rm -rf build_out && mkdir -p build_out
docker compose up --build
docker compose down
