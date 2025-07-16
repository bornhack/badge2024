use embassy_executor::Spawner;
use embassy_net::{Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_wifi::{
    EspWifiController,
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiState},
};

pub const MAX_CONNECTIONS: usize = 4;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub(crate) fn init_wifi(
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
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
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
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static>>) {
    runner.run().await
}
