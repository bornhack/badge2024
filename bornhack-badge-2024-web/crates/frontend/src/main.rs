use ev::Event;
use feature_creep_types::{Command, Message};
use leptos::*;
use leptos_use::{core::ConnectionReadyState, use_websocket, UseWebsocketReturn};

fn main() {
    mount_to_body(|| App())
}

#[component]
fn App() -> impl IntoView {
    let (read_input_hostname, write_input_hostname) = create_signal::<String>("".to_string());
    let (read_hostname, write_hostname) = create_signal::<Option<String>>(None);

    view! {
        <div class="flex flex-col gap-2 items-center py-8">
            <div>
                <input type="text"
                    placeholder="hostname"
                    class="py-2 mx-2 border border-blue rounded"
                    on:input=move |ev| {
                        write_input_hostname.set(event_target_value(&ev));
                    }
                    prop:value=read_input_hostname
                />
                <button
                    on:click=move |_ev| {
                        write_hostname.set(Some(read_input_hostname.get()));
                    }
                    class="bg-transparent hover:bg-blue-500 text-blue-700 font-semibold hover:text-white py-2 px-4 border border-blue-500 hover:border-transparent rounded"
                >
                    Connect
                </button>
            </div>
            <Show when=move || read_hostname.get().is_some()>
                {move || {
                    view! { <InitializedApp hostname=read_hostname.get().unwrap_or_default()/> }
                }}
            </Show>
        </div>
    }
}

#[component]
fn InitializedApp(hostname: String) -> impl IntoView {
    let ws_url = format!("ws://{}/ws", hostname);

    let UseWebsocketReturn {
        ready_state,
        message_bytes,
        send_bytes,
        ..
    } = use_websocket(&ws_url);

    let colors: Vec<_> = (0..16)
        .map(|_| create_signal("#00000000".to_string()))
        .collect();

    let (position, position_set) = create_signal(String::new());

    {
        let colors = colors.clone();
        create_effect(move |_| {
            if let Some(m) = message_bytes.get() {
                let res: Message = serde_json::from_slice(&m).unwrap();
                match res {
                    Message::CurrentColors(cur_colors) => {
                        for ((r, g, b), signal) in cur_colors.iter().zip(&colors) {
                            signal.1.set(format!("#{r:02x}{g:02x}{b:02x}"));
                        }
                    }
                    Message::Accelerometer([x, y, z, _t]) => {
                        position_set.set(format!("x={x:.2} y={y:.2} z={z:.2}"));
                    }
                }
            };
        });
    }

    let turn_off = {
        let send_bytes = send_bytes.clone();
        move |_| {
            for index in 0..16 {
                send_bytes(
                    serde_json::to_vec(&Command::ChangeColor {
                        index,
                        rgb: (0, 0, 0),
                    })
                    .unwrap(),
                );
            }
            send_bytes(serde_json::to_vec(&Command::QueryColors).unwrap());
        }
    };

    let random_colors = {
        let send_bytes = send_bytes.clone();
        move |_| {
            for index in 0..16 {
                send_bytes(
                    serde_json::to_vec(&Command::ChangeColor {
                        index,
                        rgb: rand::random(),
                    })
                    .unwrap(),
                );
            }
            send_bytes(serde_json::to_vec(&Command::QueryColors).unwrap());
        }
    };

    {
        let send_bytes = send_bytes.clone();
        create_effect(move |_| {
            let state = ready_state.get();
            if matches!(state, ConnectionReadyState::Open) {
                send_bytes(serde_json::to_vec(&Command::QueryColors).unwrap());
            }
        });
    }

    view! {
        <div class="flex flex-col gap-2">
            <div class="flex w-[800px] h-[325px] bg-[url('/board.png')] items-center">
                <div class="flex gap-4 ml-24 bg-[#8888] px-4 py-4 rounded-lg">
                    { (0..16).into_iter().map(|i| view!{ <Led index=i color=colors[i] send_func=send_bytes.clone() /> }).collect_view() }
                </div>
            </div>
            <div class="flex gap-2">
                <button
                    on:click=turn_off
                    class="bg-transparent hover:bg-blue-500 text-blue-700 font-semibold hover:text-white py-2 px-4 border border-blue-500 hover:border-transparent rounded"
                >
                    "turn off"
                </button>
                <button
                    on:click=random_colors
                    class="bg-transparent hover:bg-blue-500 text-blue-700 font-semibold hover:text-white py-2 px-4 border border-blue-500 hover:border-transparent rounded"
                >
                    "random colors"
                </button>
            </div>
            <div class="flex gap-2">
                <div>
                "Accelerometer"
                </div>
                { move || position.get() }
            </div>
        </div>
    }
}

#[component]
fn Led(
    index: usize,
    color: (ReadSignal<String>, WriteSignal<String>),
    send_func: impl Fn(Vec<u8>) + 'static,
) -> impl IntoView {
    let (color, set_color) = color;

    view! {
        <input
            class="rounded-full w-6 h-6"
            type="color"
            prop:value={ move ||  color.get() }
            on:input=move |ev: Event| {
                let new_value = event_target_value(&ev);
                let r = u8::from_str_radix(&new_value[1..3], 16).unwrap();
                let g = u8::from_str_radix(&new_value[3..5], 16).unwrap();
                let b = u8::from_str_radix(&new_value[5..7], 16).unwrap();
                send_func(serde_json::to_vec(&Command::ChangeColor { index: index as u8, rgb: (r,g,b) }).unwrap() );
                set_color(new_value);
        } />
    }
}
