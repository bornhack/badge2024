mod binary_json;

use feature_creep_types::{Command, Message};
use leptos::{ev::Event, prelude::*};
use leptos_use::{core::ConnectionReadyState, use_websocket, UseWebSocketReturn};

use crate::binary_json::BinaryJsonSerdeCodec;

fn main() {
    mount_to_body(App)
}

#[component]
fn App() -> impl IntoView {
    view! {
        <div class="flex flex-col gap-2 items-center py-8">
            <InitializedApp />
        </div>
    }
}

#[component]
fn InitializedApp() -> impl IntoView {
    let hostname = document().location().unwrap().host().unwrap();
    let ws_url = format!("ws://{hostname}/ws");

    let UseWebSocketReturn {
        ready_state,
        message,
        send,
        ..
    } = use_websocket::<Command, Message, BinaryJsonSerdeCodec>(&ws_url);

    let colors: Vec<_> = (0..16).map(|_| signal("#00000000".to_string())).collect();

    let (position, position_set) = signal(String::new());

    {
        let colors = colors.clone();
        Effect::new(move |_| {
            if let Some(m) = message.get() {
                match m {
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
        let send = send.clone();
        move |_| {
            for index in 0..16 {
                send(&Command::ChangeColor {
                    index,
                    rgb: (0, 0, 0),
                });
            }
            send(&Command::QueryColors);
        }
    };

    let random_colors = {
        let send = send.clone();
        move |_| {
            for index in 0..16 {
                send(&Command::ChangeColor {
                    index,
                    rgb: rand::random(),
                });
            }
            send(&Command::QueryColors);
        }
    };

    {
        let send = send.clone();
        Effect::new(move |_| {
            let state = ready_state.get();
            if matches!(state, ConnectionReadyState::Open) {
                send(&Command::QueryColors);
            }
        });
    }

    view! {
        <div class="flex flex-col gap-2">
            <div class="flex w-[800px] h-[325px] bg-[url('/board.png')] items-center">
                <div class="flex gap-4 ml-24 bg-[#8888] px-4 py-4 rounded-lg">
                    { (0..16).map(|i| view!{ <Led index=i color=colors[i] send_func=send.clone() /> }).collect_view() }
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
    send_func: impl Fn(&Command) + 'static,
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
                send_func(&Command::ChangeColor { index: index as u8, rgb: (r,g,b) });
                set_color.set(new_value);
        } />
    }
}
