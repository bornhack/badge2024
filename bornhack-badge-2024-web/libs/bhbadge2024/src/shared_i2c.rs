use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_hal::{Async, i2c::master::I2c};
use static_cell::StaticCell;

#[derive(Copy, Clone)]
pub struct SharedI2c {
    ptr: &'static Mutex<NoopRawMutex, I2c<'static, Async>>,
}

impl SharedI2c {
    pub fn new(i2c: I2c<'static, Async>) -> Self {
        static MEMORY: StaticCell<Mutex<NoopRawMutex, I2c<'static, Async>>> = StaticCell::new();
        let ptr = MEMORY.init(Mutex::new(i2c));
        Self { ptr }
    }

    pub async fn write(&self, addr: u8, bytes: &[u8]) -> Result<(), esp_hal::i2c::master::Error> {
        self.ptr.lock().await.write_async(addr, bytes).await
    }

    pub async fn read(
        &self,
        addr: u8,
        buffer: &mut [u8],
    ) -> Result<(), esp_hal::i2c::master::Error> {
        self.ptr.lock().await.read_async(addr, buffer).await
    }

    pub async fn write_read(
        &self,
        addr: u8,
        bytes: &[u8],
        buffer: &mut [u8],
    ) -> Result<(), esp_hal::i2c::master::Error> {
        self.ptr
            .lock()
            .await
            .write_read_async(addr, bytes, buffer)
            .await
    }
}
