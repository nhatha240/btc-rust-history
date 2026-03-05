use bytes::{Bytes, BytesMut};
use prost::Message;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtoError {
    #[error("Protobuf encoding error: {0}")]
    Encode(#[from] prost::EncodeError),

    #[error("Protobuf decoding error: {0}")]
    Decode(#[from] prost::DecodeError),
}

/// Helper to encode any Prost message into a `Bytes` object.
pub fn to_bytes<M: Message>(msg: &M) -> Result<Bytes, ProtoError> {
    let mut buf = BytesMut::with_capacity(msg.encoded_len());
    msg.encode(&mut buf)?;
    Ok(buf.freeze())
}

/// Helper to decode any Prost message from a byte slice.
pub fn from_bytes<M: Message + Default>(data: &[u8]) -> Result<M, ProtoError> {
    M::decode(data).map_err(Into::into)
}
