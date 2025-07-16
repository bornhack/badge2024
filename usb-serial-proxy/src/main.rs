#![no_std]
#![no_main]

use embassy_executor::{Spawner, task};
use embassy_time::Timer;
use embedded_io_async::{Read, Write};
use esp_hal::{
    Async,
    clock::CpuClock,
    uart::UartTx,
    usb_serial_jtag::{UsbSerialJtag, UsbSerialJtagRx},
};
use esp_hal_embassy::main;

const READ_BUF_SIZE: usize = 64;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    esp_hal::system::software_reset()
}

#[main]
async fn main(spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let (usb_rx, mut usb_tx) = UsbSerialJtag::new(peripherals.USB_DEVICE)
        .into_async()
        .split();

    let u1_rx: esp_hal::peripherals::GPIO0<'static> = peripherals.GPIO0;
    let u1_tx: esp_hal::peripherals::GPIO1<'static> = peripherals.GPIO1;

    let config = esp_hal::uart::Config::default()
        .with_rx(esp_hal::uart::RxConfig::default().with_fifo_full_threshold(READ_BUF_SIZE as u16));

    let (mut uart_rx, uart_tx) = esp_hal::uart::Uart::new(peripherals.UART0, config)
        .unwrap()
        .with_tx(u1_tx)
        .with_rx(u1_rx)
        .into_async()
        .split();

    spawner.spawn(sender(uart_tx, usb_rx)).unwrap();

    let mut buf = [0; READ_BUF_SIZE];
    let mut s = heapless::String::<64>::new();
    loop {
        match uart_rx.read_async(&mut buf).await {
            Ok(0) => {
                Timer::after_millis(50).await;
            }
            Err(e) => {
                use core::fmt::Write;
                s.clear();
                let _ = write!(&mut s, "Error: {e:?}\n");
                let Ok(_) = usb_tx.write_all(&s.as_bytes()).await;
                let Ok(()) = usb_tx.flush().await;
                Timer::after_millis(50).await;
            }
            Ok(n) => {
                let Ok(_) = usb_tx.write_all(&buf[..n]).await;
                let Ok(()) = usb_tx.flush().await;
            }
        }
    }
}

#[task]
async fn sender(mut uart_tx: UartTx<'static, Async>, mut usb_rx: UsbSerialJtagRx<'static, Async>) {
    let mut buf = [0; READ_BUF_SIZE];
    loop {
        let Ok(n) = usb_rx.read(&mut buf).await;
        if n == 0 {
            Timer::after_millis(50).await;
            continue;
        }
        if uart_tx.write_all(&buf[..n]).await.is_err() {
            Timer::after_millis(50).await;
        }
        if uart_tx.flush_async().await.is_err() {
            Timer::after_millis(50).await;
        }
    }
}
