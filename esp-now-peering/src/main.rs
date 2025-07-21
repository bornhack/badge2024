#![no_std]
#![no_main]

#[macro_use]
mod macros;

use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use embassy_time::Timer;
use esp_backtrace as _;
use esp_hal::{clock::CpuClock, rng::Rng, timer::timg::TimerGroup};
use esp_println::println;
use esp_wifi::{
    EspWifiController,
    esp_now::{EspNowSender, PeerInfo},
};

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) -> ! {
    esp_println::logger::init_logger_from_env();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let mut rng = Rng::new(peripherals.RNG);

    esp_hal_embassy::init(timg1.timer0);

    let esp_wifi_ctrl = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timg0.timer0, rng).unwrap()
    );
    let (mut controller, interfaces) =
        esp_wifi::wifi::new(esp_wifi_ctrl, peripherals.WIFI).unwrap();

    controller.set_mode(esp_wifi::wifi::WifiMode::Sta).unwrap();
    controller.start().unwrap();

    let esp_now = interfaces.esp_now;

    println!("ESP-NOW version: {:?}", esp_now.version().unwrap());

    let (manager, sender, mut receiver) = esp_now.split();

    let sender = mk_static!(Mutex<NoopRawMutex, EspNowSender<'static>>, Mutex::new(sender));

    Timer::after_millis((rng.random() % 1000) as u64).await;

    spawner.spawn(broadcast_task(rng, sender)).unwrap();
    loop {
        let r = receiver.receive_async().await;
        let mut s = heapless::String::<32>::new();
        for &c in r.data() {
            for e in core::ascii::escape_default(c) {
                let _ = s.push(e as char);
            }
        }
        println!(
            "Got packet: src={:x?} dst={:x?} data={:?}",
            r.info.src_address, r.info.dst_address, s
        );
        if r.info.dst_address == esp_wifi::esp_now::BROADCAST_ADDRESS && r.data() == b"peering test"
        {
            if !manager.peer_exists(&r.info.src_address) {
                manager
                    .add_peer(PeerInfo {
                        peer_address: r.info.src_address,
                        lmk: None,
                        channel: None,
                        encrypt: false,
                        interface: esp_wifi::esp_now::EspNowWifiInterface::Sta,
                    })
                    .unwrap();
            }
            let status = sender
                .lock()
                .await
                .send_async(&r.info.src_address, b"HELLO!")
                .await;
            println!("Status from sending hello: {status:?}");
        }
    }
}

#[embassy_executor::task]
async fn broadcast_task(mut rng: Rng, sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>) {
    loop {
        let status = sender
            .lock()
            .await
            .send_async(&esp_wifi::esp_now::BROADCAST_ADDRESS, b"peering test")
            .await;
        println!("Status from sending ping: {status:?}");
        Timer::after_millis(1000 + (rng.random() % 1000) as u64).await;
    }
}
