# Copyright 2020-2021 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

FROM rust:1.55.0-bullseye

RUN \
    rustup component add \
        clippy
