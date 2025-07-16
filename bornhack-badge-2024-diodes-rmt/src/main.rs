#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::Level,
    rmt::{PulseCode, Rmt, TxChannelAsync, TxChannelConfig, TxChannelCreator},
    rng::Rng,
    time::Rate,
};
use esp_hal_embassy::main;
use esp_println::println;

const NUM_PIXELS: usize = 16;

#[derive(Copy, Clone, Debug)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
}

impl Pixel {
    const BLACK: Pixel = Pixel { r: 0, g: 0, b: 0 };
}

type PixelArray = [Pixel; NUM_PIXELS];

// Ideally we would write all of the pulsecodes at the same time
// but the RMT only has space for up to 48 pulses, so we split up
// the pulse codes by pixel. We store 25 pulsecodes, so we have
// one extra for the end code.
type PulseCodeArray = [[u32; 25]; NUM_PIXELS];

#[main]
async fn main(_spawner: Spawner) {
    esp_println::logger::init_logger_from_env();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    println!("Embassy initialized!");

    Timer::after(Duration::from_secs(1)).await;

    let mut rng = Rng::new(peripherals.RNG);

    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .unwrap()
        .into_async();

    let mut channel = rmt
        .channel0
        .configure_tx(
            peripherals.GPIO10,
            TxChannelConfig::default()
                .with_clk_divider(1)
                .with_idle_output_level(Level::Low)
                .with_idle_output(true)
                .with_carrier_modulation(false),
        )
        .unwrap();

    let mut pixels: PixelArray = [Pixel::BLACK; NUM_PIXELS];
    let mut pulsecodes: PulseCodeArray = [[PulseCode::empty(); 25]; NUM_PIXELS];

    loop {
        for p in &mut pixels {
            let d = rng.random();
            p.r = (d as u8) & 0x7;
            p.g = ((d >> 8) as u8) & 0x7;
            p.b = ((d >> 16) as u8) & 0x7;
        }

        for (pixel, pulsecode) in pixels.iter_mut().zip(&mut pulsecodes) {
            pulsecode[0..8].copy_from_slice(&pixel_to_pulsecodes(pixel.g));
            pulsecode[8..16].copy_from_slice(&pixel_to_pulsecodes(pixel.r));
            pulsecode[16..24].copy_from_slice(&pixel_to_pulsecodes(pixel.b));
        }

        for pulsecode in &pulsecodes {
            // This code has an await-point between transmitting individual pixels
            // If some other task takes over and hogs the CPU for too long, then
            // we won't have time to start the next pulsecode at the right time.
            // If this happens, we will see glitches in our pulse array.
            //
            // You can check if this is what happens in your code by recording
            // the time you schedule your pulsecodes using the system clock.
            //
            // You can compare the timings with the ones stated in https://wp.josh.com/2014/05/13/ws2812-neopixels-are-not-so-finicky-once-you-get-to-know-them/
            //
            // Alternatives if you have this problem:
            // * Make sure the task that hogs the cpu does not run when this code is active
            // * Use a different peripheral such as SPI to communicate with the ws2812b.
            //
            // Using SPI has the advantage that you can use DMA to feed the peripheral, which
            // means you can avoid await-points in the middle of your transmission
            channel.transmit(pulsecode).await.unwrap();
        }

        Timer::after(Duration::from_millis(100)).await;
    }
}

fn pixel_to_pulsecodes(byte: u8) -> [u32; 8] {
    // This follows the timings from https://wp.josh.com/2014/05/13/ws2812-neopixels-are-not-so-finicky-once-you-get-to-know-them/
    let zero: u32 = PulseCode::new(
        Level::High,
        // 350 ns * 80 MHz = 28 ticks
        28,
        Level::Low,
        // 600 ns * 80 MHz = 48 ticks
        48,
    );
    let one: u32 = PulseCode::new(
        Level::High,
        // 700 ns * 80 MHz = 56 ticks
        56,
        Level::Low,
        // 600 ns * 80 MHz = 48 ticks
        48,
    );
    let mut bits = core::array::from_fn(|i| if byte & (1 << i) != 0 { one } else { zero });
    bits.reverse();
    bits
}
