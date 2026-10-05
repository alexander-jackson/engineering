#!/usr/bin/env bash

openssl rsautl -pubin -inkey public.key -encrypt | base64
