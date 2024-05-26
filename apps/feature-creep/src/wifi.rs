use embassy_executor::Spawner;
use embassy_net::{Config, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::Clocks,
    peripheral::Peripheral,
    peripherals::{RNG, SYSTIMER},
    rng::Rng,
};
use esp_println::println;
use esp_wifi::{
    initialize,
    wifi::{
        ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
        WifiState,
    },
    EspWifiInitFor,
};

use crate::{mk_static, webserver::WEB_TASK_POOL_SIZE};

pub type Stack = embassy_net::Stack<WifiDevice<'static, WifiStaDevice>>;

pub async fn init(
    spawner: &Spawner,
    client_config: ClientConfiguration,
    clocks: &Clocks<'_>,
    systimer: impl Peripheral<P = SYSTIMER>,
    rng: impl Peripheral<P = RNG>,
    radio_clocks: esp_hal::peripherals::RADIO_CLK,
    wifi: esp_hal::peripherals::WIFI,
) -> &'static Stack {
    let timer = esp_hal::timer::systimer::SystemTimer::new(systimer).alarm0;

    let mut rng = Rng::new(rng);
    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    let init = initialize(EspWifiInitFor::Wifi, timer, rng, radio_clocks, clocks).unwrap();

    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();

    let config = Config::dhcpv4(Default::default());

    // Init network stack
    let stack = &*mk_static!(
        Stack,
        Stack::new(
            wifi_interface,
            config,
            mk_static!(
                StackResources<{ WEB_TASK_POOL_SIZE + 1 }>,
                StackResources::<{ WEB_TASK_POOL_SIZE + 1 }>::new()
            ),
            seed
        )
    );

    spawner.spawn(connection(client_config, controller)).ok();
    spawner.spawn(net_task(&stack)).ok();

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    stack
}

#[embassy_executor::task]
async fn connection(config: ClientConfiguration, mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        match esp_wifi::wifi::get_wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(config.clone());
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack) {
    stack.run().await
}
