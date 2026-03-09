#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "order_side", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbOrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "order_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbOrderType {
    Market,
    Limit,
    StopMarket,
    StopLimit,
    TakeProfit,
    TakeProfitMarket,
    TrailingStopMarket,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "order_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbOrderStatus {
    New,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "time_in_force", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbTimeInForce {
    Gtc,
    Ioc,
    Fok,
    Gtx,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "order_event_type", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbOrderEventType {
    Submitted,
    Acknowledged,
    PartiallyFilled,
    Filled,
    Canceled,
    Rejected,
    Expired,
    ReplaceRequested,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "position_side", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbPositionSide {
    Long,
    Short,
    Both,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq, Copy)]
#[sqlx(type_name = "strat_status", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbStratStatus {
    Running,
    Paused,
    Halted,
    Error,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, sqlx::Type, PartialEq, Copy)]
#[sqlx(type_name = "strat_mode", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbStratMode {
    Live,
    Paper,
    Shadow,
}
