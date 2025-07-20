#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use bhbadge2024::ws2812b::Ws2812b;
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::{
    clock::ClockControl, gpio::Io, interrupt::Priority, peripherals::Peripherals, prelude::*,
    rmt::Rmt, rng::Rng, system::SystemControl, timer::timg::TimerGroup,
};
use esp_hal_embassy::InterruptExecutor;
use static_cell::StaticCell;

// Hue is assume to be in the range [0, 6];
fn hsv_to_rgb(hp: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = s * v;
    let mut x = c * (1.0 - (hp % 2.0 - 1.0));
    if x < 0.0 {
        x = -x;
    }
    let m = v - c;
    let mut r = 0.0;
    let mut g = 0.0;
    let mut b = 0.0;
    if hp <= 1.0 {
        r = c;
        g = x;
    } else if hp <= 2.0 {
        r = x;
        g = c;
    } else if hp <= 3.0 {
        g = c;
        b = x;
    } else if hp <= 4.0 {
        g = x;
        b = c;
    } else if hp <= 5.0 {
        r = x;
        b = c;
    } else {
        r = c;
        b = x;
    }
    r += m;
    g += m;
    b += m;
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

fn random_hue(rng: &mut Rng) -> f32 {
    const SIX_OVER_U32_MAXINT: f32 = 1.3969838622484784e-09;
    rng.random() as f32 * SIX_OVER_U32_MAXINT
}

#[main]
async fn main(_spawner: Spawner) {
    let peripherals = Peripherals::take();
    let system = SystemControl::new(peripherals.SYSTEM);
    let clocks = ClockControl::max(system.clock_control).freeze();

    let timer_group0 = TimerGroup::new_async(peripherals.TIMG0, &clocks);
    esp_hal_embassy::init(&clocks, timer_group0);

    static EXECUTOR: StaticCell<InterruptExecutor<1>> = StaticCell::new();
    let executor = EXECUTOR.init(InterruptExecutor::new(
        system.software_interrupt_control.software_interrupt1,
    ));
    let high_priority_spawner = executor.start(Priority::Priority2);

    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let rmt = Rmt::new_async(peripherals.RMT, 80.MHz(), &clocks).unwrap();

    let ws2812b = Ws2812b::new(&high_priority_spawner, rmt.channel0, io.pins.gpio10);
    let mut rng = Rng::new(peripherals.RNG);

    let mut i = 0;
    loop {
        Timer::after_millis(100).await;
        ws2812b.set_pixel(i, hsv_to_rgb(random_hue(&mut rng), 1.0, 0.05));
        i = (i + 1) % 16;
    }
}
