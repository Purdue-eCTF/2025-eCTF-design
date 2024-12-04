#!/bin/bash
mkdir -p ~/.ssh
printf "\nHost cfpi.neilhommes.xyz\n" >> ~/.ssh/config
echo "ProxyCommand ./cloudflared access ssh --hostname %h" >> ~/.ssh/config
printf "\nHost cfgb.neilhommes.xyz\n" >> ~/.ssh/config
echo "ProxyCommand ./cloudflared access ssh --hostname %h " >> ~/.ssh/config
printf "You can now run ssh ectf@cfpi.neilhommes.xyz to connect to the server Pass: ectf\n"
printf "You can now run ssh gsamide@cfgb.neilhommes.xyz to connect to the server Pass: b01lers\n"
