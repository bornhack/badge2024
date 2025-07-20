use crate::wifi::Stack;
use bhbadge2024::{lis2dh12::F32x3, ws2812b::Ws2812b};
use embassy_executor::Spawner;
use embassy_futures::select::Either;
use embassy_sync::{
    blocking_mutex::raw::NoopRawMutex,
    pubsub::{PubSubChannel, Subscriber, WaitResult},
};
use embassy_time::{Duration, Timer};
use esp_println::println;
use feature_creep_types::{Command, Message};
use picoserve::{
    extract::State,
    io::embedded_io_async,
    response::{ws, IntoResponse},
    routing::get,
    Router,
};

pub const WEB_TASK_POOL_SIZE: usize = 3;

struct WebsocketHandler {
    ws2812b: Ws2812b,
    subscriber: Subscriber<'static, NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>,
}

impl ws::WebSocketCallback for WebsocketHandler {
    async fn run<R: embedded_io_async::Read, W: embedded_io_async::Write<Error = R::Error>>(
        mut self,
        mut rx: ws::SocketRx<R>,
        mut tx: ws::SocketTx<W>,
    ) -> Result<(), W::Error> {
        let mut buffer = [0; 1024];
        let mut buffer2 = [0; 1024];

        let close_reason = loop {
            let msg = embassy_futures::select::select(
                rx.next_message(&mut buffer),
                self.subscriber.next_message(),
            )
            .await;
            match msg {
                Either::First(Ok(ws::Message::Text(_data))) => {
                    break Some((1003, "Only binary data accepted"))
                }
                Either::First(Ok(ws::Message::Binary(data))) => {
                    match serde_json_core::from_slice(data) {
                        Ok((Command::ChangeColor { index, rgb }, consumed))
                            if data.len() == consumed && index < 16 =>
                        {
                            self.ws2812b.set_pixel(index as usize, rgb);
                            // There was a race condition. We didn't understand it. Now it is no longer here. ¯\_(ツ)_/¯
                            Timer::after_micros(50).await;
                        }
                        Ok((Command::QueryColors, _consumed)) => {
                            let mut res = [(0u8, 0u8, 0u8); 16];
                            self.ws2812b.with_frame_buffer(|f| {
                                for (i, pix) in f.raw_mut().iter().enumerate() {
                                    res[i].0 = pix[1];
                                    res[i].1 = pix[0];
                                    res[i].2 = pix[2];
                                }
                            });

                            let len = serde_json_core::to_slice(
                                &Message::CurrentColors(res),
                                &mut buffer2,
                            )
                            .unwrap();

                            tx.send_binary(&buffer2[..len]).await.ok();
                        }
                        Ok((command, consumed)) => {
                            println!(
                                "Unexpected command: consumed={}, data.len()=={}, command={:?}",
                                consumed,
                                data.len(),
                                command
                            );
                        }
                        Err(e) => {
                            println!("Could not parse command: {e:?}",);
                        }
                    }
                }
                Either::First(Ok(ws::Message::Close(reason))) => {
                    println!("Websocket close reason: {reason:?}");
                    break None;
                }
                Either::First(Ok(ws::Message::Ping(data))) => {
                    tx.send_pong(data).await?;
                }
                Either::First(Ok(ws::Message::Pong(_))) => (),
                Either::First(Err(err)) => {
                    println!("Websocket Error: {err:?}");

                    let code = match err {
                        ws::ReadMessageError::Io(err) => return Err(err),
                        ws::ReadMessageError::ReadFrameError(_)
                        | ws::ReadMessageError::MessageStartsWithContinuation
                        | ws::ReadMessageError::UnexpectedMessageStart => 1002,
                        ws::ReadMessageError::ReservedOpcode(_) => 1003,
                        ws::ReadMessageError::TextIsNotUtf8 => 1007,
                    };

                    break Some((code, "Websocket Error"));
                }
                Either::Second(WaitResult::Lagged(_)) => (),
                Either::Second(WaitResult::Message(m)) => {
                    let len = serde_json_core::to_slice(
                        &Message::Accelerometer([m.0.x, m.0.y, m.0.z, m.1]),
                        &mut buffer2,
                    )
                    .unwrap();

                    tx.send_binary(&buffer2[..len]).await.ok();
                }
            };
        };

        tx.close(close_reason).await
    }
}

async fn websocket(
    State(state): State<&'static AppState>,
    upgrade: picoserve::response::WebSocketUpgrade,
) -> impl IntoResponse {
    upgrade
        .on_upgrade(WebsocketHandler {
            ws2812b: state.ws2812b,
            subscriber: state.channel.subscriber().unwrap(),
        })
        .await
}

fn make_app() -> picoserve::Router<AppRouter, &'static AppState> {
    // static INDEX: &str = include_str!("../dist/index.html");
    Router::new()
        // .route("/", get(|| webserver_file::File::html(INDEX)))
        // .route("/index.html", get(|| webserver_file::File::html(INDEX)))
        // .route(
        //     "/frontend.js",
        //     get(|| webserver_file::File::javascript(include_str!("../dist/frontend.js"))),
        // )
        // .route(
        //     "/tailwind.css",
        //     get(|| webserver_file::File::css(include_str!("../dist/tailwind.css"))),
        // )
        // .route(
        //     "/frontend_bg.wasm",
        //     get(|| {
        //         webserver_file::File::with_content_type(
        //             "application/wasm",
        //             include_bytes!("../dist/frontend_bg.wasm"),
        //         )
        //     }),
        // )
        // .route(
        //     "/board.png",
        //     get(|| {
        //         webserver_file::File::with_content_type(
        //             "image/png",
        //             include_bytes!("../dist/board.png"),
        //         )
        //     }),
        // )
        .route("/ws", get(websocket))
}

pub struct AppState {
    pub ws2812b: Ws2812b,
    pub channel: PubSubChannel<NoopRawMutex, (F32x3, f32), 1, WEB_TASK_POOL_SIZE, 1>,
}

type AppRouter = impl picoserve::routing::PathRouter<&'static AppState>;
type App = picoserve::Router<AppRouter, &'static AppState>;

pub async fn init(spawner: &Spawner, stack: &'static Stack, app_state: &'static AppState) {
    // We cannot use static_cell::make_static because of https://github.com/embassy-rs/static-cell/issues/16
    static APP: static_cell::StaticCell<App> = static_cell::StaticCell::new();
    let app = APP.init(make_app());

    static CONFIG: static_cell::StaticCell<picoserve::Config<embassy_time::Duration>> =
        static_cell::StaticCell::new();
    let config = CONFIG.init(
        picoserve::Config::new(picoserve::Timeouts {
            start_read_request: Some(Duration::from_secs(5)),
            read_request: Some(Duration::from_secs(1)),
            write: Some(Duration::from_secs(1)),
        })
        .keep_connection_alive(),
    );

    for id in 0..WEB_TASK_POOL_SIZE {
        spawner.must_spawn(web_task(id, stack, app, config, app_state));
    }
}

#[embassy_executor::task(pool_size = WEB_TASK_POOL_SIZE)]
async fn web_task(
    id: usize,
    stack: &'static Stack,
    app: &'static App,
    config: &'static picoserve::Config<Duration>,
    state: &'static AppState,
) -> ! {
    let port = 80;
    let mut tcp_rx_buffer = [0; 1024];
    let mut tcp_tx_buffer = [0; 1024];
    let mut http_buffer = [0; 2048];

    picoserve::listen_and_serve_with_state(
        id,
        app,
        config,
        stack,
        port,
        &mut tcp_rx_buffer,
        &mut tcp_tx_buffer,
        &mut http_buffer,
        &state,
    )
    .await
}
