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

org='ezanmoto'
proj='dpnd'
build_img="$org/$proj.build"
cross_img="$org/$proj.cross"

# We build `dpnd.build:latest` first because `cross.Dockerfile` depends on it.
bash scripts/docker_rbuild.sh \
    "$build_img" \
    "latest" \
    --file='build.Dockerfile' \
    scripts

bash scripts/docker_rbuild.sh \
    "$cross_img" \
    "latest" \
    --file='cross.Dockerfile' \
    scripts

vol_name="$org.$proj.cargo_cache"
vol_dir='/cargo'

docker run \
    --rm \
    --mount="type=volume,src=$vol_name,dst=$vol_dir" \
    "$cross_img:latest" \
    chmod 0777 "$vol_dir"

work_dir='/app'

run_in_env() {
    # We run the `cross` build environment as `root` instead of using
    # `--user=$(id --user):$(id --group)` because version 0.2.1 of `cross`
    # requires the active user to have a username; see
    # <https://github.com/rust-embedded/cross/pull/505> for more details.
    docker run \
        --rm \
        --mount="type=volume,src=$vol_name,dst=$vol_dir" \
        --env="CARGO_HOME=$vol_dir" \
        --mount="type=bind,src=$(pwd),dst=$work_dir" \
        --workdir="$work_dir" \
        --group-add=docker \
        --mount='type=bind,src=/var/run/docker.sock,dst=/var/run/docker.sock' \
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
