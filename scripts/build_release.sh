# Copyright 2021 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

# `$0 <target>` builds a release binary of `dpnd` for the given `target`.
# Supported targets are those supported by
# [cross](https://github.com/rust-embedded/cross#supported-targets).

set -o errexit

if [ $# -ne 1 ] ; then
    echo "usage: $0 <target>" >&2
    exit 1
fi

target="$1"

cross_img='dpnd.cross'

# We build `dpnd.build:latest` first because `cross.Dockerfile` depends on it.
bash scripts/docker_rbuild.sh \
    "dpnd.build" \
    "latest" \
    --file='scripts/build.Dockerfile' \
    scripts

bash scripts/docker_rbuild.sh \
    "$cross_img" \
    "latest" \
    --file='scripts/cross.Dockerfile' \
    scripts

docker run \
    --rm \
    --mount='type=volume,src=dpnd_cargo_cache,dst=/cargo' \
    "$cross_img:latest" \
    chmod 0777 /cargo

run_in_env() {
    # We run the `cross` build environment as `root` instead of using
    # `--user=$(id -u):$(id -g)` because version 0.2.1 of `cross` requires the
    # active user to have a username; see
    # <https://github.com/rust-embedded/cross/pull/505> for more details.
    docker run \
        --rm \
        --mount='type=volume,src=dpnd_cargo_cache,dst=/cargo' \
        --env='CARGO_HOME=/cargo' \
        --mount="type=bind,src=$(pwd),dst=/app" \
        --workdir='/app' \
        --group-add=docker \
        --mount="type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock" \
        "$cross_img:latest" \
        "$@"
}

run_in_env \
    cross test \
        --target "$target" \
        -- \
        --show-output \
        --test-threads=1

run_in_env \
    cross build \
        --release \
        --target "$target"
