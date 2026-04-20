#!/usr/bin/env bash

mkdir ../certificates

mkcert -install
mkcert localhost 127.0.0.1 ::1 --cert-file ../certificates/local-fullchain.pem --key-file ../certificates/local-privkey.pem
