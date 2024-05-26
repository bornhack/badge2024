#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![allow(long_running_const_eval)]

pub mod webserver;
mod webserver_file;
pub mod wifi;

use bhbadge2024::{
    lis2dh12::{F32x3, Lis2dh12},
    shared_i2c,
    ws2812b::Ws2812b,
};
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    pubsub::{PubSubChannel, Publisher},
};
use embassy_time::Timer;
use esp_hal::{
    clock::ClockControl, gpio::Io, i2c::I2C, interrupt::Priority, peripherals::Peripherals,
    prelude::*, rmt::Rmt, system::SystemControl, timer::timg::TimerGroup,
};
use esp_hal_embassy::InterruptExecutor;
use esp_wifi::wifi::{AuthMethod, ClientConfiguration};
use static_cell::StaticCell;
use webserver::{AppState, WEB_TASK_POOL_SIZE};

#[macro_export]
macro_rules! mk_static {
    ($t:ty,$val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

#[main]
async fn main(spawner: Spawner) {
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

    let ws2812b = Ws2812b::new(&high_priority_spawner, rmt.channel0, io.pins.gpio10);

    let stack = wifi::init(
        &spawner,
        ClientConfiguration {
            ssid: "bornhack".try_into().unwrap(),
            auth_method: AuthMethod::None,
            ..Default::default()
        },
        &clocks,
        peripherals.SYSTIMER,
        peripherals.RNG,
        peripherals.RADIO_CLK,
        peripherals.WIFI,
    )
    .await;

    let channel = PubSubChannel::<NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>::new();
    let app_state: &'static AppState = mk_static!(AppState, AppState { ws2812b, channel });
    let publisher = app_state.channel.publisher().unwrap();

    webserver::init(&spawner, stack, app_state).await;

    let shared_i2c = shared_i2c::SharedI2c::new(I2C::new_async(
        peripherals.I2C0,
        io.pins.gpio6,
        io.pins.gpio7,
        4.kHz(),
        &clocks,
    ));

    init_lis2dh12(&spawner, shared_i2c, publisher).await;

    // Badge:
    //   sda/scl: io6/io7
    //   LIS2DH12:
    //     datasheet: https://www.st.com/resource/en/datasheet/lis2dh12.pdf
    //     address: 0x19
    //     freq: 0-400kHz
    //   NT3H2x11:
    //     datasheet: https://www.nxp.com/docs/en/data-sheet/NT3H2111_2211.pdf
    //     address: 0x55
    //     freq: 0-400kHz
    //
    // Rust board:
    //   sda/scl: io10/io8
    //   SHTC3:
    //     datasheet: https://sensirion.com/media/documents/643F9C8E/63A5A436/Datasheet_SHTC3.pdf
    //     address: 0x70
    //     freq: 0-1MHz
    //   ICM-42670-P:
    //     datasheet: https://invensense.tdk.com/wp-content/uploads/2021/07/DS-000451-ICM-42670-P-v1.0.pdf
    //     address: 0x68
    //     freq: 0-1MHz
    //

    loop {
        Timer::after_micros(500).await;
    }
}

async fn init_lis2dh12(
    spawner: &Spawner,
    shared_i2c: shared_i2c::SharedI2c,
    publisher: Publisher<'static, NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>,
) {
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

    spawner.must_spawn(read_accelerometer(lis2dh12, publisher));
}

#[embassy_executor::task]
async fn read_accelerometer(
    mut lis2dh12: Lis2dh12,
    publisher: Publisher<'static, NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>,
) {
    loop {
        let dir = lis2dh12.accel_norm().await.unwrap();
        let temperature = lis2dh12.get_temp_outf().await.unwrap();
        publisher.publish_immediate((dir, temperature + 20.0));
        Timer::after_millis(500).await;
    }
}
