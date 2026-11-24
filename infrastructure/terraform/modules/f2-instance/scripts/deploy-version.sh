#!/usr/bin/env bash

set -euxo pipefail

if [ -z "$${1:-}" ]; then
	echo "Provide a version to deploy: ./deploy-version.sh <version>"
	exit 1
fi

version="$1"

# Pull the new version down
docker pull alexanderjackson/f2:$${version}

# Terminate all the existing containers
docker ps -q | xargs -r docker rm -f

# Start Vector
docker run -d --network internal -v /var/run/docker.sock:/var/run/docker.sock -v /mnt/vector:/var/lib/vector -v /home/ubuntu/vector.yaml:/etc/vector/vector.yaml timberio/vector:${vector_tag}

# Start `f2` again
docker run -d -v /var/run/docker.sock:/var/run/docker.sock -v /tmp:/tmp -v /mnt:/mnt --network internal ${port_flags} alexanderjackson/f2:$${version} -- --config s3://${config_bucket}/${config_key}
