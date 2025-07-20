#!/usr/bin/env bash

trunk build --release
cargo run --release -p feature-creep "$@"
