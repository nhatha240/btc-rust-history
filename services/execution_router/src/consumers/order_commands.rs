use anyhow::{Context, Result};
use hft_proto::encode::from_bytes;
use hft_proto::oms::OrderCommand;

pub fn decode_order_command(payload: &[u8]) -> Result<OrderCommand> {
    from_bytes::<OrderCommand>(payload).context("decode OrderCommand failed")
}
