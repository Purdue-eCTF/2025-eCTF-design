#!/bin/sh

files=""

if [ -z "$1" ] || [ -z "$2" ]
then
  echo "usage: hostname upload_folder"
  exit 1
fi

if [ -e ap.img ]
then
  files="ap.img"
fi

if [ -e compa.img ]
then
  files="$files compa.img"
fi

if [ -e compb.img ]
then
  files="$files compb.img"
fi

if [ -n "$files" ]
then
  rsync -av --progress --rsync-path="mkdir -p ~/CI/rpi/upload/$2 && rsync" $files $1:~/CI/rpi/upload/$2
else
  echo "neither ap.img or comp.img do not exist"
fi
