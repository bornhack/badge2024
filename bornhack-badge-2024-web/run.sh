#!/usr/bin/env bash

export RUSTFLAGS="-Clink-arg=-Tlinkall.x -Clink-arg=-Trom_functions.x -Cforce-frame-pointers"
export ESP_LOGLEVEL="info"

cargo run --release --target riscv32imc-unknown-none-elf -Z build-std="core,alloc" -p $@
