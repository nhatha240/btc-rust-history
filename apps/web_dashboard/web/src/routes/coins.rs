use axum::{
    extract::{Path, State},
    routing::{get, put, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, FromRow};
use chrono::{DateTime, Utc};
use tower_http::cors::CorsLayer;

#[derive(Serialize, Deserialize, FromRow, Clone, Debug)]
pub struct Coin {
    pub id: i64,
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Deserialize, Debug)]
pub struct CreateCoinReq {
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    #[serde(default = "default_active")]
    pub is_active: bool,
}

fn default_active() -> bool {
    true
}

#[derive(Deserialize, Debug)]
pub struct UpdateCoinReq {
    pub is_active: Option<bool>,
    pub base_asset: Option<String>,
    pub quote_asset: Option<String>,
}

/// Router setup for the coins endpoints
pub fn router(pool: PgPool) -> Router {
    Router::new()
        .route("/", get(list_coins).post(create_coin))
        .route("/{symbol}", put(update_coin).delete(delete_coin))
        .with_state(pool)
        .layer(CorsLayer::permissive())
}

/// Check if MOCK_DATA is enabled for dummy responses
fn is_mock() -> bool {
    std::env::var("MOCK_DATA").map(|v| v == "1" || v == "true").unwrap_or(false)
}

// -----------------------------------------------------------------------------
// Handlers
// -----------------------------------------------------------------------------

async fn list_coins(State(pool): State<PgPool>) -> Json<Vec<Coin>> {
    if is_mock() {
        return Json(vec![
            Coin {
                id: 1,
                symbol: "BTCUSDT".to_string(),
                base_asset: "BTC".to_string(),
                quote_asset: "USDT".to_string(),
                is_active: true,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
            Coin {
                id: 2,
                symbol: "ETHUSDT".to_string(),
                base_asset: "ETH".to_string(),
                quote_asset: "USDT".to_string(),
                is_active: false,
                created_at: Utc::now(),
                updated_at: Utc::now(),
            },
        ]);
    }

    let query = "SELECT * FROM coins ORDER BY symbol ASC";
    match sqlx::query_as::<_, Coin>(query).fetch_all(&pool).await {
        Ok(coins) => Json(coins),
        Err(e) => {
            tracing::error!("Failed to fetch coins: {:?}", e);
            Json(vec![])
        }
    }
}

async fn create_coin(
    State(pool): State<PgPool>,
    Json(payload): Json<CreateCoinReq>,
) -> Json<serde_json::Value> {
    if is_mock() {
        return Json(serde_json::json!({"status": "ok", "message": "Mock coin created"}));
    }

    let query = r#"
        INSERT INTO coins (symbol, base_asset, quote_asset, is_active)
        VALUES ($1, $2, $3, $4)
        RETURNING *
    "#;

    match sqlx::query_as::<_, Coin>(query)
        .bind(&payload.symbol)
        .bind(&payload.base_asset)
        .bind(&payload.quote_asset)
        .bind(payload.is_active)
        .fetch_one(&pool)
        .await
    {
        Ok(coin) => Json(serde_json::json!({
            "status": "ok",
            "data": coin
        })),
        Err(e) => {
            tracing::error!("Failed to create coin: {:?}", e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}

async fn update_coin(
    State(pool): State<PgPool>,
    Path(symbol): Path<String>,
    Json(payload): Json<UpdateCoinReq>,
) -> Json<serde_json::Value> {
    if is_mock() {
        return Json(serde_json::json!({"status": "ok", "message": "Mock coin updated"}));
    }

    // Check if any field is provided
    if payload.is_active.is_none() && payload.base_asset.is_none() && payload.quote_asset.is_none() {
        return Json(serde_json::json!({
            "status": "error",
            "message": "No fields to update"
        }));
    }
    // Instead, let's just do a simpler query using COALESCE if we just had all three, but let's build using pg query builder or query builder
    // To avoid complex builder here, we will use query! or just rely on a simple update block
    let update_query = r#"
        UPDATE coins 
        SET 
            is_active = COALESCE($1, is_active),
            base_asset = COALESCE($2, base_asset),
            quote_asset = COALESCE($3, quote_asset)
        WHERE symbol = $4
        RETURNING *
    "#;

    match sqlx::query_as::<_, Coin>(update_query)
        .bind(payload.is_active)
        .bind(payload.base_asset)
        .bind(payload.quote_asset)
        .bind(&symbol)
        .fetch_one(&pool)
        .await
    {
        Ok(coin) => Json(serde_json::json!({
            "status": "ok",
            "data": coin
        })),
        Err(e) => {
            tracing::error!("Failed to update coin {}: {:?}", symbol, e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}

async fn delete_coin(
    State(pool): State<PgPool>,
    Path(symbol): Path<String>,
) -> Json<serde_json::Value> {
    if is_mock() {
        return Json(serde_json::json!({"status": "ok", "message": "Mock coin deleted"}));
    }

    let query = "DELETE FROM coins WHERE symbol = $1";
    match sqlx::query(query).bind(&symbol).execute(&pool).await {
        Ok(result) if result.rows_affected() > 0 => Json(serde_json::json!({
            "status": "ok",
            "message": "Coin deleted"
        })),
        Ok(_) => Json(serde_json::json!({
            "status": "error",
            "message": "Coin not found"
        })),
        Err(e) => {
            tracing::error!("Failed to delete coin {}: {:?}", symbol, e);
            Json(serde_json::json!({
                "status": "error",
                "message": e.to_string()
            }))
        }
    }
}
