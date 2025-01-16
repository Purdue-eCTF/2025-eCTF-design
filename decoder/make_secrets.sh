#!/bin/sh

cd $(dirname $0)

poetry run python ectf_tools/build_depl.py -d .
