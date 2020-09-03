# Copyright 2020 Sean Kelleher. All rights reserved.
# Use of this source code is governed by an MIT
# licence that can be found in the LICENCE file.

FROM rust:1.45.2-stretch

RUN \
    rustup component add \
        clippy
