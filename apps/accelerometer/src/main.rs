#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use bhbadge2024::{lis2dh12::Lis2dh12, shared_i2c, ws2812b::Ws2812b};
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::{
    clock::ClockControl, gpio::Io, i2c::I2C, interrupt::Priority, peripherals::Peripherals,
    prelude::*, rmt::Rmt, system::SystemControl, timer::timg::TimerGroup,
};
use esp_hal_embassy::InterruptExecutor;
use esp_println::println;
use static_cell::StaticCell;

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timer_group0);

    static EXECUTOR: StaticCell<InterruptExecutor<1>> = StaticCell::new();
    let executor = EXECUTOR.init(InterruptExecutor::new(
        system.software_interrupt_control.software_interrupt1,
    ));
    let high_priority_spawner = executor.start(Priority::Priority2);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let rmt = Rmt::new_async(peripherals.RMT, 80.MHz(), &clocks).unwrap();

    let _ws2812b = Ws2812b::new(&high_priority_spawner, rmt.channel0, io.pins.gpio10);

    let shared_i2c = shared_i2c::SharedI2c::new(I2C::new_async(
        peripherals.I2C0,
        io.pins.gpio6,
        io.pins.gpio7,
        4.kHz(),
        &clocks,
    ));

    let mut lis2dh12 = Lis2dh12::new(
        shared_i2c,
        bhbadge2024::lis2dh12::SlaveAddr::Alternative(true),
    )
    .await
    .unwrap();

    lis2dh12.reset().await.unwrap();
    lis2dh12
        .set_odr(bhbadge2024::lis2dh12::Odr::Hz400)
        .await
        .unwrap();
    lis2dh12
        .set_mode(bhbadge2024::lis2dh12::Mode::Normal)
        .await
        .unwrap();
    lis2dh12
        .set_fs(bhbadge2024::lis2dh12::FullScale::G16)
        .await
        .unwrap();
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
