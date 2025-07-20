use leptos::server::codee::{Decoder, Encoder};

pub struct BinaryJsonSerdeCodec;

impl<T: serde::Serialize> Encoder<T> for BinaryJsonSerdeCodec {
    type Error = serde_json::Error;
    type Encoded = Vec<u8>;

    fn encode(msg: &T) -> Result<Self::Encoded, Self::Error> {
        serde_json::to_vec(msg)
    }
}

impl<T: serde::de::DeserializeOwned> Decoder<T> for BinaryJsonSerdeCodec {
    type Error = serde_json::Error;
    type Encoded = [u8];

    fn decode(data: &Self::Encoded) -> Result<T, Self::Error> {
        serde_json::from_slice(data)
    }
}
