#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embedded_io_async::{Read, Write};
use esp_hal::{clock::CpuClock, usb_serial_jtag::UsbSerialJtag};
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

    let mut usb_serial: UsbSerialJtag<'static, _> =
        UsbSerialJtag::new(peripherals.USB_DEVICE).into_async();

    let mut buf = [0; READ_BUF_SIZE];
    loop {
        let Ok(n) = usb_serial.read(&mut buf).await;
        let Ok(_) = usb_serial.write_all(&buf[..n]).await;
        // Add some extra garbage when the user enters ! so we can see
        // that something is actually happening
        if buf[..n].contains(&b'!') {
            let Ok(_) = usb_serial.write_all(b"!!").await;
        }
        let Ok(()) = usb_serial.flush().await;
    }
}
