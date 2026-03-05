// Shared database types and enums
// For now, we use standard types, but we can add custom mapping here.

#[derive(Debug, serde::Serialize, serde::Deserialize, sqlx::Type)]
#[sqlx(type_name = "order_side", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DbOrderSide {
    Buy,
    Sell,
}
