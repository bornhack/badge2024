#![no_std]
#![no_main]
#![allow(long_running_const_eval)]

extern crate alloc;

use esp_alloc as _;
use esp_backtrace as _;
use esp_hal_embassy::main;

#[macro_use]
pub mod macros;
pub mod lis2dh12;
pub mod shared_i2c;
pub mod webserver;
mod webserver_file;
pub mod wifi;
pub mod ws2812b;

use crate::lis2dh12::{F32x3, Lis2dh12};
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    pubsub::{PubSubChannel, Publisher},
};
use embassy_time::Timer;
use esp_hal::{clock::CpuClock, i2c::master::I2c, rng::Rng, time::Rate, timer::timg::TimerGroup};
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

esp_bootloader_esp_idf::esp_app_desc!();

#[main]
async fn main(spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    esp_alloc::heap_allocator!(size: 72 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let rng = Rng::new(peripherals.RNG);

    esp_hal_embassy::init(timg1.timer0);

    let stack = wifi::init_wifi(&spawner, timg0.timer0, rng, peripherals.WIFI).await;

    let ws2812b = ws2812b::init_ws2812b(
        spawner,
        peripherals.SPI2,
        peripherals.GPIO10,
        peripherals.DMA_CH0,
    );

    let channel = PubSubChannel::<NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>::new();
    let app_state: &'static AppState = mk_static!(AppState, AppState { ws2812b, channel });
    let publisher = app_state.channel.publisher().unwrap();

    webserver::init(&spawner, stack, app_state).await;

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
    let mut lis2dh12 = Lis2dh12::new(shared_i2c, lis2dh12::SlaveAddr::Alternative(true))
        .await
        .unwrap();

    lis2dh12.reset().await.unwrap();
    lis2dh12.set_odr(lis2dh12::Odr::Hz400).await.unwrap();
    lis2dh12.set_mode(lis2dh12::Mode::Normal).await.unwrap();
    lis2dh12.set_fs(lis2dh12::FullScale::G16).await.unwrap();
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
