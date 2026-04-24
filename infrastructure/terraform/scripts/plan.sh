#!/usr/bin/env bash
set -euo pipefail

source "$(dirname "$0")/secrets.sh"
cd "$(dirname "$0")/.."

terraform plan -no-color -input=false "$@"
