#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig},
};
use esp_hal_embassy::main;
use esp_println::println;

#[main]
async fn main(spawner: Spawner) -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(CpuClock::max()));

    let timer0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG1);
    esp_hal_embassy::init(timer0.timer0);

    let select = Input::new(peripherals.GPIO9, InputConfig::default());
    let up = Input::new(
        peripherals.GPIO2,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );
    let down = Input::new(
        peripherals.GPIO8,
        InputConfig::default().with_pull(esp_hal::gpio::Pull::Up),
    );

    spawner.spawn(input_handler(select, "Select")).unwrap();
    spawner.spawn(input_handler(up, "Up")).unwrap();
    spawner.spawn(input_handler(down, "Down")).unwrap();

    core::future::pending().await
}

#[embassy_executor::task(pool_size = 3)]
async fn input_handler(mut input: Input<'static>, name: &'static str) {
    loop {
        input.wait_for_low().await;
        println!("{name} pushed");
        Timer::after_millis(20).await;
        input.wait_for_high().await;
        println!("{name} released");
        Timer::after_millis(20).await;
    }
}
