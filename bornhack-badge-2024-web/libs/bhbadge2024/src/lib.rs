#![no_std]
#![feature(type_alias_impl_trait)]

use esp_backtrace as _;

pub mod lis2dh12;
pub mod shared_i2c;
pub mod ws2812b;
