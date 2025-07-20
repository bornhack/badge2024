#!/usr/bin/env bash

echo $USERNAME
echo $PASSWORD

trunk build --release
cargo run --release -p feature-creep --target riscv32imc-unknown-none-elf "$@"
