#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embedded_io_async::Write;
use esp_hal::clock::CpuClock;
use esp_hal_embassy::main;

const READ_BUF_SIZE: usize = 64;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let config = esp_hal::uart::Config::default()
        .with_rx(esp_hal::uart::RxConfig::default().with_fifo_full_threshold(READ_BUF_SIZE as u16));

    let mut uart0 = esp_hal::uart::Uart::new(peripherals.UART0, config)
        .unwrap()
        .with_tx(peripherals.GPIO21)
        .with_rx(peripherals.GPIO20)
        .into_async();

    let mut buf = [0; READ_BUF_SIZE];
    loop {
        let Ok(n) = uart0.read_async(&mut buf).await else {
            continue;
        };
        let Ok(_) = uart0.write_all(&buf[..n]).await else {
            continue;
        };
        let Ok(()) = uart0.flush_async().await else {
            continue;
        };
    }
}
