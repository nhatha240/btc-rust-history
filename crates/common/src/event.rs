pub const TOPIC_CANDLES_1M: &str = "md.candles.1m.v1";
pub const TOPIC_FEATURE_STATE: &str = "features.state.v1";
pub const TOPIC_SIGNAL_DECISION: &str = "signals.decision.v1";

pub fn key_symbol(exchange: &str, symbol: &str) -> String {
    format!("{exchange}|{symbol}")
}
pub fn key_state(exchange: &str, symbol: &str, interval: &str) -> String {
    format!("{exchange}|{symbol}|{interval}")
}
