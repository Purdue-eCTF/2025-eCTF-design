#!/bin/bash
./make_secrets.sh
./build_ap.sh
./build_comp.sh
if [ $1 -eq 1 ]; then
./upload.sh ectf@cfpi.neilhommes.xyz $2
ssh ectf@cfpi.neilhommes.xyz<< EOF
  ls;
  cd CI/rpi/ ;
  ./nix-boot.sh all 123456 $2
EOF
fi
if [ $1 -eq 2 ]; then
./upload.sh gsamide@cfgb.neilhommes.xyz $2
ssh gsamide@cfgb.neilhommes.xyz<< EOF
  ls;
  cd CI/rpi/ ;
  ./nix-boot.sh all 123456 $2
EOF
fi
