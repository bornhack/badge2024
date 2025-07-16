# BornHack 2024 ESP32-C3 Firmware Examples

This repository contains a collection of small, practical firmware examples for the ESP32-C3 microcontroller. Some examples include code specific to the badge made for BornHack 2024/2025, but most can be used as general reference projects for ESP32-C3 development in Rust.

## How to Use

- Each subdirectory is a standalone example.
- You can copy any example out of this repository and use it independently.
- To build and run an example, navigate into its directory and run:

```sh
cargo run --release
```

## Examples Included

- Accelerometer
- Buttons
- Diodes (RMT and SPI)
- ESP-NOW (broadcast and peering)
- UART (announcer and echo server)
- USB serial (echo server and proxy)
- WiFi (echo server and scanner)
