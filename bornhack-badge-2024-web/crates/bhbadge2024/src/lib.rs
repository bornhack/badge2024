#![no_std]

use esp_backtrace as _;

#[macro_use]
pub mod lis2dh12;
pub mod shared_i2c;
pub mod ws2812b;
