#!/bin/sh

cd $(dirname $0)

echo "usage: <filename> <channel_number> <start_timestamp> <end_timestamp>"

python -m ectf25_design.gen_subscription secrets/global.secrets $1 0xdeadbeef $3 $4 $2
