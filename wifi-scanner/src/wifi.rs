use embassy_time::{Duration, Timer};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_wifi::{EspWifiController, wifi::ScanConfig};

pub(crate) async fn run_scanner(
    timer: esp_hal::timer::timg::Timer<'static>,
    rng: Rng,
    wifi: WIFI<'static>,
) -> ! {
    let init = &*mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer, rng).unwrap()
    );

    let (mut controller, _) = esp_wifi::wifi::new(&init, wifi).unwrap();

    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    controller.set_mode(esp_wifi::wifi::WifiMode::Sta).unwrap();
    loop {
        if !matches!(controller.is_started(), Ok(true)) {
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }

        println!("Starting scan...");
        match controller
            .scan_with_config_async(ScanConfig::default())
            .await
        {
            Ok(res) => {
                println!("Got {} results", res.len());
                for r in res {
                    println!(
                        "ssid={:?} strength={} channel={} auth={:?}",
                        r.ssid, r.signal_strength, r.channel, r.auth_method
                    );
                }
            }
            Err(e) => {
                println!("Error while scanning {e:?}");
            }
        }
        Timer::after(Duration::from_millis(5000)).await
    }
}
