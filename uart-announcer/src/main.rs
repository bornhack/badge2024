#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_io_async::Write;
use esp_hal::clock::CpuClock;
use esp_hal_embassy::main;

const READ_BUF_SIZE: usize = 64;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

const ANNOUNCEMENT: &str = env!("ANNOUNCEMENT");

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let config = esp_hal::uart::Config::default()
        .with_rx(esp_hal::uart::RxConfig::default().with_fifo_full_threshold(READ_BUF_SIZE as u16));

    let mut uart0 = esp_hal::uart::Uart::new(peripherals.UART0, config)
        .unwrap()
        .with_tx(peripherals.GPIO0)
        .with_rx(peripherals.GPIO1)
        .into_async();

    loop {
        let Ok(()) = uart0.write_all(ANNOUNCEMENT.as_bytes()).await else {
            continue;
        };
        let Ok(()) = uart0.flush_async().await else {
            continue;
        };

        Timer::after_millis(200).await;
    }
}
