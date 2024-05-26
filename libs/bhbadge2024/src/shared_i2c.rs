use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_hal::{i2c::I2C, peripherals::I2C0, Async};
use static_cell::StaticCell;

#[derive(Copy, Clone)]
pub struct SharedI2c {
    ptr: &'static Mutex<NoopRawMutex, I2C<'static, I2C0, Async>>,
}

impl SharedI2c {
    pub fn new(i2c: I2C<'static, I2C0, Async>) -> Self {
        static MEMORY: StaticCell<Mutex<NoopRawMutex, I2C<'static, I2C0, Async>>> =
            StaticCell::new();
        let ptr = MEMORY.init(Mutex::new(i2c));
        Self { ptr }
    }

    pub async fn write(&self, addr: u8, bytes: &[u8]) -> Result<(), esp_hal::i2c::Error> {
        self.ptr.lock().await.write(addr, bytes).await
    }

    pub async fn read(&self, addr: u8, buffer: &mut [u8]) -> Result<(), esp_hal::i2c::Error> {
        self.ptr.lock().await.read(addr, buffer).await
    }

    pub async fn write_read(
        &self,
        addr: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), esp_hal::i2c::Error> {
        self.ptr.lock().await.write_read(addr, bytes, buffer).await
    }
}
