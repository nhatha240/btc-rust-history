use anyhow::Result;
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
struct ExchangeInfo { symbols: Vec<SymbolInfo> }

#[derive(Debug, Deserialize, Clone)]
struct SymbolInfo {
    symbol: String,
    status: String,
    #[serde(rename="isSpotTradingAllowed")]
    is_spot: Option<bool>,
    #[serde(rename="quoteAsset")]
    quote: Option<String>,
}

pub async fn symbols_all(only_spot: bool, quotes: Option<Vec<String>>) -> Result<Vec<String>> {
    let info: ExchangeInfo = reqwest::Client::new()
        .get("https://api.binance.com/api/v3/exchangeInfo")
        .send().await?
        .json().await?;

    let qset: Option<HashSet<String>> =
        quotes.map(|v| v.into_iter().map(|s| s.to_uppercase()).collect());

    let mut out = Vec::new();
    for s in info.symbols.into_iter() {
        if s.status != "TRADING" { continue; }
        if only_spot && !s.is_spot.unwrap_or(false) { continue; }
        if let (Some(set), Some(qa)) = (&qset, &s.quote) {
            if !set.contains(&qa.to_uppercase()) { continue; }
        }
        out.push(s.symbol);
    }
    Ok(out)
}
