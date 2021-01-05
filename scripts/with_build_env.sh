# Copyright 2020-2021 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

# `$0` runs a command in the build environment.

set -o errexit

build_img='dpnd.build'

bash scripts/docker_rbuild.sh \
    "$build_img" \
    "latest" \
    --file='scripts/build.Dockerfile' \
    scripts

docker run \
    --rm \
    --mount='type=volume,src=dpnd_cargo_cache,dst=/cargo' \
    "$build_img:latest" \
    chmod 0777 /cargo

docker run \
    --interactive \
    --tty \
    --rm \
    --mount='type=volume,src=dpnd_cargo_cache,dst=/cargo' \
    --env='CARGO_HOME=/cargo' \
    --user="$(id -u):$(id -g)" \
    --mount="type=bind,src=$(pwd),dst=/app" \
    --workdir='/app' \
    "$build_img:latest" \
    "$@"
