use core::cell::RefCell;

use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use esp_hal::{
    dma, dma_buffers,
    spi::{self, master::SpiDmaBus},
    time::Rate,
    Async,
};

const NUM_PIXELS: usize = 16;
// 3 colors per pixel, 1 byte per color, 4 bits of spi data per bit of color data
const NUM_SPI_BYTES: usize = NUM_PIXELS * 3 * 4;

#[derive(Copy, Clone, Debug)]
pub struct Pixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Pixel {
    pub const BLACK: Pixel = Pixel { r: 0, g: 0, b: 0 };
}

type PixelArray = [Pixel; NUM_PIXELS];
type PulseCodeArray = [u8; NUM_SPI_BYTES];

// This corresponds to 350ns of high followed by 1050s of low
const ZERO_PULSE: u8 = 0b1000;
// This corresponds to 700 ns of high followed by 700 ns of low
const ONE_PULSE: u8 = 0b1100;

type BufferMutex = Mutex<CriticalSectionRawMutex, RefCell<PixelArray>>;
type ActivationSignal = Signal<CriticalSectionRawMutex, ()>;

#[derive(Copy, Clone)]
pub struct Ws2812b {
    activation_signal: &'static ActivationSignal,
    frame_buffer: &'static BufferMutex,
}

impl Ws2812b {
    pub fn set_pixel(&self, index: usize, pixel: Pixel) {
        assert!(index < NUM_PIXELS);
        self.frame_buffer.lock(|pixels| {
            pixels.borrow_mut()[index] = pixel;
            self.activation_signal.signal(());
        });
    }

    pub fn set_pixels(&self, f: impl FnOnce(&mut PixelArray)) {
        self.frame_buffer.lock(|pixels| {
            f(&mut pixels.borrow_mut());
            self.activation_signal.signal(());
        });
    }
}

pub fn init_ws2812b(
    spawner: embassy_executor::Spawner,
    spi2: esp_hal::peripherals::SPI2<'static>,
    gpio10: esp_hal::peripherals::GPIO10<'static>,
    dma_ch0: esp_hal::peripherals::DMA_CH0<'static>,
) -> Ws2812b {
    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(NUM_SPI_BYTES);
    let dma_rx_buf = dma::DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = dma::DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    // The frequency 2857kHz was chosen because 1/2857kHz ~= 350.018 ns, which is pretty close to
    // our desired pulse length
    //
    // However since this exact frequency is not supported by the spi and instead esp-hal will
    // choose the closest matching frequency. This closest frequency is 80MHz/28, which corresponds
    // to bit-length of exactly 350 ns.
    let spi_config = spi::master::Config::default().with_frequency(Rate::from_khz(2857));

    let spidma = spi::master::Spi::new(spi2, spi_config)
        .unwrap()
        .with_mosi(gpio10)
        .with_dma(dma_ch0)
        .with_buffers(dma_rx_buf, dma_tx_buf)
        .into_async();

    let mutex = mk_static!(
        BufferMutex,
        Mutex::new(RefCell::new([Pixel::BLACK; NUM_PIXELS]))
    );
    let activation_signal = mk_static!(ActivationSignal, Signal::new());

    spawner
        .spawn(handler(spidma, mutex, activation_signal))
        .unwrap();

    Ws2812b {
        activation_signal,
        frame_buffer: mutex,
    }
}

#[embassy_executor::task]
async fn handler(
    mut spidma: SpiDmaBus<'static, Async>,
    frame_buffer: &'static BufferMutex,
    activation_signal: &'static ActivationSignal,
) {
    let pulsecodes = mk_static!(PulseCodeArray, [0; NUM_SPI_BYTES]);
    loop {
        activation_signal.wait().await;
        frame_buffer.lock(|pixels| {
            activation_signal.reset();
            for (pixel, pulsecode) in pixels
                .borrow_mut()
                .iter_mut()
                .zip(pulsecodes.chunks_mut(3 * 4))
            {
                pulsecode[0..4].copy_from_slice(&pixel_to_pulsecodes(pixel.g));
                pulsecode[4..8].copy_from_slice(&pixel_to_pulsecodes(pixel.r));
                pulsecode[8..12].copy_from_slice(&pixel_to_pulsecodes(pixel.b));
            }
        });
        spidma.write_async(&*pulsecodes).await.unwrap();
    }
}

fn pixel_to_pulsecodes(byte: u8) -> [u8; 4] {
    const PULSECODES: [u8; 4] = [
        ZERO_PULSE << 4 | ZERO_PULSE,
        ZERO_PULSE << 4 | ONE_PULSE,
        ONE_PULSE << 4 | ZERO_PULSE,
        ONE_PULSE << 4 | ONE_PULSE,
    ];
    [
        PULSECODES[((byte >> 6) & 0b11) as usize],
        PULSECODES[((byte >> 4) & 0b11) as usize],
        PULSECODES[((byte >> 2) & 0b11) as usize],
        PULSECODES[((byte >> 0) & 0b11) as usize],
    ]
}
