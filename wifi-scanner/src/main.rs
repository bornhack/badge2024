#![no_std]
#![no_main]

#[macro_use]
mod macros;
mod wifi;

use embassy_executor::Spawner;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rng::Rng, timer::timg::TimerGroup};

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let rng = Rng::new(peripherals.RNG);

    esp_hal_embassy::init(timg1.timer0);

    wifi::run_scanner(timg0.timer0, rng, peripherals.WIFI).await;
}
