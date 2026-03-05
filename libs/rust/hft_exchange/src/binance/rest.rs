use crate::binance::signer::sign;
use crate::binance::types::{BinanceOrderAck, BinancePosition};
use crate::rate_limit::RateLimiter;
use anyhow::{Context, Result};
use chrono::Utc;
use hft_proto::oms::OrderCommand;
use reqwest::Client;
use std::sync::Arc;

pub struct BinanceRestClient {
    client: Client,
    api_key: String,
    api_secret: String,
    base_url: String,
    rate_limiter: Arc<RateLimiter>,
}

impl BinanceRestClient {
    pub fn new(
        api_key: String,
        api_secret: String,
        base_url: String,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        Self {
            client: Client::new(),
            api_key,
            api_secret,
            base_url,
            rate_limiter,
        }
    }

    pub async fn place_order(&self, cmd: &OrderCommand) -> Result<BinanceOrderAck> {
        self.rate_limiter.acquire(1.0).await;

        let timestamp = Utc::now().timestamp_millis();
        let side = if cmd.side == 1 { "BUY" } else { "SELL" };
        let mut query = format!(
            "symbol={}&side={}&type=MARKET&quantity={}&timestamp={}",
            cmd.symbol, side, cmd.qty, timestamp
        );
        let signature = sign(&self.api_secret, &query);
        query.push_str(&format!("&signature={}", signature));

        let url = format!("{}/fapi/v1/order?{}", self.base_url, query);
        let resp = self
            .client
            .post(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to send Binance order")?;

        let status = resp.status();
        let text = resp.text().await.context("Failed to get response text")?;
        if !status.is_success() {
            anyhow::bail!("Binance order error: status={}, body={}", status, text);
        }

        serde_json::from_str(&text).context("Failed to parse BinanceOrderAck")
    }

    pub async fn cancel_order(&self, symbol: &str, order_id: i64) -> Result<()> {
        self.rate_limiter.acquire(1.0).await;

        let timestamp = Utc::now().timestamp_millis();
        let mut query = format!(
            "symbol={}&orderId={}&timestamp={}",
            symbol, order_id, timestamp
        );
        let signature = sign(&self.api_secret, &query);
        query.push_str(&format!("&signature={}", signature));

        let url = format!("{}/fapi/v1/order?{}", self.base_url, query);
        let resp = self
            .client
            .delete(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to cancel Binance order")?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("Binance cancel error: status={}, body={}", status, body);
        }

        Ok(())
    }

    pub async fn fetch_positions(&self) -> Result<Vec<BinancePosition>> {
        self.rate_limiter.acquire(1.0).await;

        let timestamp = Utc::now().timestamp_millis();
        let mut query = format!("timestamp={}", timestamp);
        let signature = sign(&self.api_secret, &query);
        query.push_str(&format!("&signature={}", signature));

        let url = format!("{}/fapi/v2/positionRisk?{}", self.base_url, query);
        let resp = self
            .client
            .get(&url)
            .header("X-MBX-APIKEY", &self.api_key)
            .send()
            .await
            .context("Failed to fetch Binance positions")?;

        let status = resp.status();
        let text = resp.text().await.context("Failed to get response text")?;
        if !status.is_success() {
            anyhow::bail!("Binance position error: status={}, body={}", status, text);
        }

        serde_json::from_str(&text).context("Failed to parse BinancePosition list")
    }
}
