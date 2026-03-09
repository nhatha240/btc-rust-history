use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use clickhouse::Client;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct SignalParams {
    pub symbol: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub side: Option<String>, // "LONG" or "SHORT"
}

#[derive(serde::Serialize)]
pub struct SignalResponse {
    pub symbol: String,
    pub ts: i64,
    pub side: String,
    pub strategy_name: String,
    pub price: f64,
    pub confidence: f32,
    pub reason: String,
}

pub fn router(ch_client: Client) -> Router {
    Router::new()
        .route("/", get(handle_list_signals))
        .with_state(ch_client)
}

async fn handle_list_signals(
    State(ch_client): State<Client>,
    Query(params): Query<SignalParams>,
) -> Json<Vec<SignalResponse>> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let side_filter = params.side.as_deref();

    // Build dynamic SQL query
    let mut query = "SELECT symbol, ts, side, strategy_name, price, confidence, reason FROM signals".to_string();
    let mut conditions = Vec::new();

    if let Some(symbol) = &params.symbol {
        conditions.push(format!("symbol = '{}'", symbol));
    }

    if let Some(side) = side_filter {
        if side.eq_ignore_ascii_case("LONG") {
            conditions.push("side = 'LONG'".to_string());
        } else if side.eq_ignore_ascii_case("SHORT") {
            conditions.push("side = 'SHORT'".to_string());
        }
    }

    if !conditions.is_empty() {
        query.push_str(" WHERE ");
        query.push_str(&conditions.join(" AND "));
    }

    query.push_str(&format!(" ORDER BY ts DESC LIMIT {} OFFSET {}", limit, offset));

    // Execute query
    let result = ch_client.query(&query)
        .fetch_all::<(String, i64, String, String, f64, f32, String)>()
        .await;

    match result {
        Ok(rows) => {
            let signals: Vec<SignalResponse> = rows.into_iter().map(|(symbol, ts, side, strategy_name, price, confidence, reason)| SignalResponse {
                symbol,
                ts,
                side,
                strategy_name,
                price,
                confidence,
                reason,
            }).collect();

            Json(signals)
        },
        Err(e) => {
            tracing::error!("Failed to query signals: {}", e);
            Json(vec![])
        }
    }
}