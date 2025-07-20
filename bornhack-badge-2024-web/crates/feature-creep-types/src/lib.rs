#![no_std]
//use core::prelude::rust_2021::derive;
//use serde::{Deserialize, Serialize};
//
//#[derive(Deserialize, Serialize)]
//pub enum Command {
//    ChangeColor(&'static str),
//}
//

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Update {}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Command {
    #[serde(rename = "c")]
    ChangeColor {
        #[serde(rename = "i")]
        index: u8,
        #[serde(rename = "c")]
        rgb: (u8, u8, u8),
    },
    QueryColors,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Message {
    #[serde(rename = "c")]
    CurrentColors([(u8, u8, u8); 16]),
    #[serde(rename = "a")]
    Accelerometer([f32; 4]),
}
