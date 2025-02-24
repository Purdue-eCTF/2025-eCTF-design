#!/bin/sh

cd $(dirname $0)

sudo python -m ectf25.utils.tester --secrets secrets/global.secrets --port /dev/ttyACM0 stdin
