use core::cell::RefCell;

use embassy_executor::SendSpawner;
use embassy_sync::{
    blocking_mutex::{raw::CriticalSectionRawMutex, Mutex},
    signal::Signal,
};
use embassy_time::Timer;
use esp_hal::{
    gpio::OutputPin,
    peripheral::Peripheral,
    rmt::{
        asynch::TxChannelAsync, ChannelCreator, PulseCode, TxChannelConfig, TxChannelCreatorAsync,
    },
    Async,
};
use micromath::F32Ext;
use static_cell::ConstStaticCell;

const PIXEL_COUNT: usize = 16;

struct CommunicationState {
    frame_buffer: BufferMutex,
    activation_signal: ActivationSignal,
}

type BufferMutex = Mutex<CriticalSectionRawMutex, RefCell<[[u8; 3]; PIXEL_COUNT]>>;
type ActivationSignal = Signal<CriticalSectionRawMutex, ()>;
// Ideally we would write all of the pulsecodes at the same time
// but the RMT only has space for up to 48 pulses, so we split up
// the pulse codes by pixel. We store 25 pulsecodes, so we have
// one extra for the end code.
type PulseCodeArray = [[u32; 25]; PIXEL_COUNT];

#[derive(Copy, Clone)]
pub struct Ws2812b {
    state: &'static CommunicationState,
}

pub struct FrameBuffer<'a> {
    frame_buffer: &'a mut [[u8; 3]; PIXEL_COUNT],
}

impl<'a> FrameBuffer<'a> {
    /// Sets a single pixel.
    ///
    /// ### Example
    ///
    /// ```
    /// frame_buffer.set_pixel(0, (40, 100, 255));
    /// ```
    pub fn set_pixel(&mut self, index: usize, rgb: (u8, u8, u8)) {
        let pixel = &mut self.frame_buffer[index];
        let (r, g, b) = rgb;
        pixel[0] = g;
        pixel[1] = r;
        pixel[2] = b;
    }

    /// Gets raw access to the frame_buffer. Note that the pixels are stored in grb format.
    pub fn raw_mut(&mut self) -> &mut [[u8; 3]; PIXEL_COUNT] {
        &mut self.frame_buffer
    }
}

const CHANNEL: u8 = 0;

impl Ws2812b {
    /// Initializes the ws2812b driver.
    ///
    /// ## Example
    ///
    /// ```
    /// let rmt = Rmt::new_async(peripherals.RMT, 80.MHz(), &clocks).unwrap();
    /// let ws2812b = Ws2812b::new(&spawner, rmt.channel0, io.pins.gpio10);
    /// ```
    ///
    pub fn new<'d, P>(
        spawner: &SendSpawner,
        channel_creator: ChannelCreator<Async, 0>,
        pin: impl Peripheral<P = P> + 'd,
    ) -> Self
    where
        P: OutputPin,
    {
        static STATE: ConstStaticCell<CommunicationState> =
            ConstStaticCell::new(CommunicationState {
                frame_buffer: Mutex::new(RefCell::new([[0; 3]; PIXEL_COUNT])),
                activation_signal: Signal::new(),
            });
        static PULSECODES: ConstStaticCell<PulseCodeArray> =
            ConstStaticCell::new([[0u32; 25]; PIXEL_COUNT]);

        let state = STATE.take();
        let pulsecodes = PULSECODES.take();

        let channel = channel_creator
            .configure(
                pin,
                TxChannelConfig {
                    clk_divider: 1,
                    idle_output_level: false,
                    idle_output: true,
                    carrier_modulation: false,
                    carrier_high: 1,
                    carrier_low: 1,
                    carrier_level: false,
                },
            )
            .unwrap();
        spawner
            .spawn(handler(
                channel,
                &state.frame_buffer,
                &state.activation_signal,
                pulsecodes,
            ))
            .unwrap();
        state.activation_signal.signal(());
        Self { state }
    }

    /// Gets access to the frame buffer.
    ///
    /// ### Example
    ///
    /// ```
    /// ws2812b.with_frame_buffer(|mut framebuffer| {
    ///     framebuffer.set_pixel(0, (40, 100, 255));
    /// });
    /// ```
    pub fn with_frame_buffer<F, R>(&self, f: F) -> R
    where
        F: for<'a> FnOnce(&'a mut FrameBuffer<'a>) -> R,
    {
        let result = self.state.frame_buffer.lock(|frame_buffer| {
            let mut frame_buffer = frame_buffer.borrow_mut();
            f(&mut FrameBuffer {
                frame_buffer: &mut *frame_buffer,
            })
        });
        self.state.activation_signal.signal(());
        result
    }

    /// Sets a single pixel.
    ///
    /// Note that if need to set multiple pixels, then it is more effecient to use [`with_frame_buffer`].
    ///
    /// ### Example
    ///
    /// ```
    /// ws2812b.set_pixel(0, (40, 100, 255));
    /// ```
    pub fn set_pixel(&self, index: usize, rgb: (u8, u8, u8)) {
        self.with_frame_buffer(|frame_buffer| {
            frame_buffer.set_pixel(index, rgb);
        });
    }
}

type Channel = esp_hal::rmt::Channel<Async, CHANNEL>;

#[embassy_executor::task]
async fn handler(
    mut channel: Channel,
    frame_buffer: &'static BufferMutex,
    activation_signal: &'static ActivationSignal,
    pulsecodes: &'static mut PulseCodeArray,
) {
    loop {
        activation_signal.wait().await;

        const CORRECTIONS: [f32; 3] = [
            0.3 * 177.0 / 256.0,
            0.3 * 256.0 / 256.0,
            0.3 * 241.0 / 256.0,
        ];

        frame_buffer.lock(|frame_buffer| {
            let frame_buffer = frame_buffer.borrow();
            for (chunk, pulsecodes) in frame_buffer.iter().zip(pulsecodes.iter_mut()) {
                for ((b, pulsecodes), correction) in chunk
                    .iter()
                    .zip(pulsecodes.chunks_exact_mut(8))
                    .zip(CORRECTIONS)
                {
                    write_pulse_codes(*b, pulsecodes.try_into().unwrap(), correction);
                }
            }
        });

        // Send the pulsecodes to the rmt one at a time. Ideally we would write all of them at
        // once, but it does not have enough ram. We could in principle use wrapping mode, but that
        // does not work with the async interface.
        //
        // In practice this is should be fine: We will get slightly longer pauses between pulses
        // especially if another task does not yield in time, however slightly longer pauses should
        // be fine, even if it is somewhat outside the spec.
        for pulsecode in pulsecodes.iter() {
            channel.transmit(pulsecode).await.unwrap();
        }

        // Datasheet says minimum reset time is 50 microseconds.
        // some places on the internet says to wait more, but this seems to work okay
        Timer::after_micros(50).await;
    }
}

#[inline(always)]
fn write_pulse_codes(byte: u8, out: &mut [u32; 8], correction: f32) {
    // These numbers do *not* match the datasheet, they match https://github.com/karlri/esp32-rmt-ws2812b/blob/main/src/lib.rs
    // We make the zero pulses shorter and the one pulses longer. This seems to work okay
    const ZERO: PulseCode = PulseCode {
        level1: true,
        // length1: 32, // 400 ns * 80 MHz = 32 ticks
        length1: 20, // 250 ns * 80 MHz = 32 ticks
        level2: false,
        // length2: 68, // 850 ns * 80 MHz = 68 ticks
        length2: 80, // 1000 ns * 80 MHz = 68 ticks
    };
    const ONE: PulseCode = PulseCode {
        level1: true,
        // length1: 64, // 800 ns * 80 MHz = 64 ticks
        length1: 70, // 875 ns * 80 MHz = 64 ticks
        level2: false,
        // length2: 34, // 450 ns * 80 MHz = 36 ticks
        length2: 30, // 375 ns * 80 MHz = 36 ticks
    };
    // The buffer for the pulse codes is already big enough, so we only store the packed pulse codes.
    const ZERO_BITS: u32 = (0 << 31) | (80 << 16) | (1 << 15) | (20 << 0);
    const ONE_BITS: u32 = (0 << 31) | (30 << 16) | (1 << 15) | (70 << 0);

    // Sanity check that we got the conversion right.
    debug_assert_eq!(u32::from(ZERO), ZERO_BITS);
    debug_assert_eq!(u32::from(ONE), ONE_BITS);

    // Write out the bits one at a time, starting with the most significant bit.
    // This code is a bit weird-looking, because Tethys decided to do a silly
    // micro-optimization instead of writing the most readable version
    let byte = ((byte as f32) * correction).round() as u8;
    let mut byte = (byte as u32) << 24;
    for out in out {
        *out = if (byte & 0x80000000) == 0 {
            ZERO_BITS
        } else {
            ONE_BITS
        };
        byte <<= 1;
    }
}
