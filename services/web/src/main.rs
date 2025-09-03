use std::{net::SocketAddr, str::FromStr};

use axum::{
    extract::{Query, State},
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use http::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};
use tracing::{error, info, Level};

#[derive(Clone)]
struct AppState {
    // ClickHouse HTTP endpoint, ví dụ: http://clickhouse:8123
    ch_dsn: Option<String>,
}

// ===================== DTOs =====================
#[derive(Debug, Deserialize)]
struct CandleQuery {
    symbol: String,     // e.g. BTCUSDT
    interval: String,   // 1m | 5m | 15m | 1h
    limit: Option<u32>, // default 500
}

#[derive(Debug, Serialize)]
struct CandleDto {
    symbol: String,
    open_time: i64, // ms
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: f64,
    close_time: i64,
    quote_asset_volume: f64,
    number_of_trades: u64,
    taker_buy_base_asset_volume: f64,
    taker_buy_quote_asset_volume: f64,
}

#[derive(Debug, Deserialize)]
struct OrdersQuery {
    symbol: Option<String>,
    limit: Option<u32>,
}

#[derive(Debug, Serialize)]
struct OrderDto {
    id: String,
    ts: i64,        // ms
    symbol: String,
    side: String,   // BUY/SELL
    price: f64,
    qty: f64,
    status: String, // FILLED/...
    note: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    let port: u16 = std::env::var("WEB_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8080);

    let ch_dsn = std::env::var("CLICKHOUSE_DSN").ok();
    let state = AppState { ch_dsn };

    // CORS (tower-http 0.6): allow_headers nhận HeaderName (http v1)
    let cors = CorsLayer::new()
        .allow_methods([Method::GET])
        .allow_headers([CONTENT_TYPE])
        .allow_origin(Any);

    // API dưới prefix /api
    let api = Router::new()
        .route("/candles", get(get_candles))
        .route("/orders", get(get_orders))
        .with_state(state);

    // App = static + api + middlewares
    let app = Router::new()
        // WORKDIR trong Docker là /srv; copy ./static -> /srv/static
        .nest_service("/", ServeDir::new("static"))
        .nest("/api", api)
        .layer(cors)
        .layer(TraceLayer::new_for_http());

    tracing_subscriber::fmt()
        .with_target(false)
        .with_max_level(Level::INFO)
        .with_env_filter("info,tower_http=info")
        .init();

    let addr = SocketAddr::from_str(&format!("0.0.0.0:{port}"))?;
    info!(%addr, "web server starting");
    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;
    Ok(())
}

// ===================== Handlers =====================

// GET /api/candles?symbol=BTCUSDT&interval=1m&limit=500
async fn get_candles(
    State(state): State<AppState>,
    Query(q): Query<CandleQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(500).min(5000);
    let table = match q.interval.as_str() {
        "1m" => "db_trading.candles_1m_final",
        "5m" => "db_trading.candles_5m",
        "15m" => "db_trading.candles_15m",
        "1h" => "db_trading.candles_1h",
        _ => "db_trading.candles_1m_final",
    };

    if let Some(dsn) = &state.ch_dsn {
        match fetch_candles_clickhouse(dsn, table, &q.symbol, limit).await {
            Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
            Err(e) => {
                error!(error=?e, "failed to fetch candles from ClickHouse");
                (StatusCode::OK, Json::<Vec<CandleDto>>(vec![])).into_response()
            }
        }
    } else {
        (StatusCode::OK, Json::<Vec<CandleDto>>(vec![])).into_response()
    }
}

// GET /api/orders?symbol=BTCUSDT&limit=200
async fn get_orders(
    State(state): State<AppState>,
    Query(q): Query<OrdersQuery>,
) -> impl IntoResponse {
    let limit = q.limit.unwrap_or(200).min(2000);
    if let Some(dsn) = &state.ch_dsn {
        match fetch_orders_clickhouse(dsn, q.symbol.clone(), limit).await {
            Ok(rows) => (StatusCode::OK, Json(rows)).into_response(),
            Err(e) => {
                error!(error=?e, "failed to fetch orders");
                (StatusCode::OK, Json::<Vec<OrderDto>>(vec![])).into_response()
            }
        }
    } else {
        (StatusCode::OK, Json::<Vec<OrderDto>>(vec![])).into_response()
    }
}

// ===================== ClickHouse accessors =====================

async fn fetch_candles_clickhouse(
    dsn: &str,
    table: &str,
    symbol: &str,
    limit: u32,
) -> anyhow::Result<Vec<CandleDto>> {
    use clickhouse::{Client, Row};

    #[derive(Row, Deserialize, Debug)]
    struct R {
        symbol: String,
        open_time: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
        close_time: i64,
        quote_asset_volume: f64,
        number_of_trades: u64,
        taker_buy_base_asset_volume: f64,
        taker_buy_quote_asset_volume: f64,
    }

    // v0.13: khởi tạo client bằng builder API
    let client: Client = Client::default().with_url(dsn);

    let sql = format!(
        r#"
        SELECT symbol, open_time, open, high, low, close, volume,
        close_time, quote_asset_volume, number_of_trades,
        taker_buy_base_asset_volume, taker_buy_quote_asset_volume
        FROM {table}
        WHERE symbol = ?
        ORDER BY open_time DESC
        LIMIT ?
        "#
    );

    // chỉ định kiểu hàng R tại fetch::<R>() để compiler suy luận được RowCursor<R>
    let mut cursor = client
        .query(&sql)
        .bind(symbol)
        .bind(limit)
        .fetch::<R>()?;

    let mut out = Vec::with_capacity(limit as usize);
    while let Some(r) = cursor.next().await? {
        out.push(CandleDto {
            symbol: r.symbol,
            open_time: r.open_time,
            open: r.open,
            high: r.high,
            low: r.low,
            close: r.close,
            volume: r.volume,
            close_time: r.close_time,
            quote_asset_volume: r.quote_asset_volume,
            number_of_trades: r.number_of_trades,
            taker_buy_base_asset_volume: r.taker_buy_base_asset_volume,
            taker_buy_quote_asset_volume: r.taker_buy_quote_asset_volume,
        });
    }
    // đảo sang tăng dần cho chart
    out.reverse();
    Ok(out)
}

async fn fetch_orders_clickhouse(
    dsn: &str,
    symbol: Option<String>,
    limit: u32,
) -> anyhow::Result<Vec<OrderDto>> {
    use clickhouse::{Client, Row};

    #[derive(Row, Deserialize, Debug)]
    struct R {
        id: String,
        ts: i64,
        symbol: String,
        side: String,
        price: f64,
        qty: f64,
        status: String,
        note: Option<String>,
    }

    let client: Client = Client::default().with_url(dsn);

    let (sql, bind_symbol) = if symbol.as_deref().unwrap_or("").is_empty() {
        (
            r#"
            SELECT id, ts, symbol, side, price, qty, status, note
            FROM db_trading.orders
            ORDER BY ts DESC
            LIMIT ?
            "#
                .to_string(),
            false,
        )
    } else {
        (
            r#"
            SELECT id, ts, symbol, side, price, qty, status, note
            FROM db_trading.orders
            WHERE symbol = ?
            ORDER BY ts DESC
            LIMIT ?
            "#
                .to_string(),
            true,
        )
    };

    let mut q = client.query(&sql);
    if bind_symbol {
        q = q.bind(symbol.unwrap());
    }
    let mut cursor = q.bind(limit).fetch::<R>()?;

    let mut out = Vec::with_capacity(limit as usize);
    while let Some(r) = cursor.next().await? {
        out.push(OrderDto {
            id: r.id,
            ts: r.ts,
            symbol: r.symbol,
            side: r.side,
            price: r.price,
            qty: r.qty,
            status: r.status,
            note: r.note,
        });
    }
    Ok(out)
}
