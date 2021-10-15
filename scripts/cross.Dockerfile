# Copyright 2021 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

FROM dpnd.build:latest

RUN \
    curl \
        --silent \
        --show-error \
        --location \
        https://get.docker.com \
    | VERSION=20.10 \
        sh

RUN \
    cargo install \
        --version=0.2.1 cross

ENV CROSS_DOCKER_IN_DOCKER=true

# We set `TEST_DIR=/tmp` for use with `cargo test`. `/tmp` is used for the test
# directory because `cross` runs the tests in a new Docker container (not the
# one being created by this Dockerfile), so we need to choose a location that
# already exists or the tests will fail.
ENV TEST_DIR=/tmp
