#![no_std]
#![no_main]

pub mod lis2dh12;
mod shared_i2c;

use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, i2c::master::I2c, time::Rate};
use esp_println::println;

use crate::lis2dh12::Lis2dh12;

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let shared_i2c = shared_i2c::SharedI2c::new(
        I2c::new(
            peripherals.I2C0,
            esp_hal::i2c::master::Config::default()
                .with_frequency(Rate::from_khz(4))
                .with_software_timeout(esp_hal::i2c::master::SoftwareTimeout::None),
        )
        .unwrap()
        .with_sda(peripherals.GPIO6)
        .with_scl(peripherals.GPIO7)
        .into_async(),
    );

    let mut lis2dh12 = Lis2dh12::new(shared_i2c, lis2dh12::SlaveAddr::Alternative(true))
        .await
        .unwrap();

    lis2dh12.reset().await.unwrap();
    lis2dh12.set_odr(lis2dh12::Odr::Hz400).await.unwrap();
    lis2dh12.set_mode(lis2dh12::Mode::Normal).await.unwrap();
    lis2dh12.set_fs(lis2dh12::FullScale::G16).await.unwrap();
    lis2dh12.enable_axis((true, true, true)).await.unwrap();
    lis2dh12.enable_temp(true).await.unwrap();

    loop {
        Timer::after_millis(500).await;
        let dir = lis2dh12.accel_norm().await.unwrap();
        let temperature = lis2dh12.get_temp_outf().await.unwrap();
        println!(
            "x={:.2} y={:.2} z={:.2} t={:?}",
            dir.x,
            dir.y,
            dir.z,
            temperature + 20.0
        );
    }
}
