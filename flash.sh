#!/bin/sh

cd $(dirname $0)

sudo python -m ectf25.utils.flash build_out/max78000.bin /dev/ttyACM0
