use alloc::string::String;
use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_wifi::{
    wifi::{
        ClientConfiguration, Configuration, EapClientConfiguration, TtlsPhase2Method,
        WifiController, WifiDevice, WifiEvent, WifiState,
    },
    EspWifiController,
};

const WIFI_SSID: &str = env!("WIFI_SSID");
const WIFI_USERNAME: Option<&str> = option_env!("WIFI_USERNAME");
const WIFI_PASSWORD: &str = env!("WIFI_PASSWORD");

pub const MAX_CONNECTIONS: usize = 5;

pub async fn init_wifi(
    spawner: &Spawner,
    timer: esp_hal::timer::timg::Timer<'static>,
    mut rng: Rng,
    wifi: WIFI<'static>,
) -> Stack<'static> {
    let init = mk_static!(
        EspWifiController<'static>,
        esp_wifi::init(timer, rng).unwrap()
    );

    let (controller, wifi_interfaces) = esp_wifi::wifi::new(init, wifi).unwrap();

    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    println!("max connections = {MAX_CONNECTIONS}");

    // Init network stack
    let (stack, runner) = embassy_net::new(
        wifi_interfaces.sta,
        config,
        mk_static!(
            StackResources<{ 2 + MAX_CONNECTIONS }>,
            StackResources::new()
        ),
        seed,
    );

    spawner.spawn(connection(controller)).unwrap();
    spawner.spawn(net_task(runner)).unwrap();

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

    spawner.spawn(resolve_google_in_loop(stack)).unwrap();

    stack
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = if let Some(username) = WIFI_USERNAME {
                Configuration::EapClient(EapClientConfiguration {
                    ssid: String::from(WIFI_SSID),
                    auth_method: esp_wifi::wifi::AuthMethod::WPA2Enterprise,
                    username: Some(String::from(username)),
                    password: Some(String::from(WIFI_PASSWORD)),
                    ttls_phase2_method: Some(TtlsPhase2Method::Pap),
                    ..Default::default()
                })
            } else {
                Configuration::Client(ClientConfiguration {
                    ssid: String::from(WIFI_SSID),
                    auth_method: esp_wifi::wifi::AuthMethod::WPA2Personal,
                    password: String::from(WIFI_PASSWORD),
                    ..Default::default()
                })
            };

            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start_async().await.unwrap();
            println!("Wifi started!");
        }
        println!("About to connect...");

        match controller.connect_async().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_secs(90)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn resolve_google_in_loop(stack: Stack<'static>) {
    loop {
        println!(
            "Resolved google: {:?}",
            stack
                .dns_query("google.com", smoltcp::wire::DnsQueryType::A)
                .await
        );

        Timer::after_secs(2).await;
    }
}
