use clap::ArgAction;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc,SecondsFormat};
use clap::Parser;
use clickhouse::{Client as Ch, Row};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, time::Duration};
use tokio::time::sleep;
use tracing::{error, info, warn};
use anyhow::*;
use reqwest::Client;


/// CLI job: fetch Binance spot/usdm assets, join CoinGecko market caps,
/// filter by threshold, and insert snapshot rows into ClickHouse.
#[derive(Parser, Debug)]
#[command(version, about = "MarketCap snapshot -> ClickHouse")]
struct Args {
    /// ClickHouse HTTP URL (e.g., http://localhost:8123)
    #[arg(long, env = "CLICKHOUSE_URL", default_value = "http://localhost:8123")]
    ch_url: String,
    /// ClickHouse database name
    #[arg(long, env = "CLICKHOUSE_DB", default_value = "db_trading")]
    ch_db: String,
    /// ClickHouse user
    #[arg(long, env = "CLICKHOUSE_USER", default_value = "default")]
    ch_user: Option<String>,
    /// ClickHouse password
    #[arg(long, env = "CLICKHOUSE_PASSWORD", default_value = "root@123")]
    ch_password: Option<String>,

    /// Minimum market cap in USD
    #[arg(long, env = "MIN_MARKET_CAP_USD", default_value_t = 100_000_000f64)]
    min_market_cap_usd: f64,

    /// Include Binance Spot
    #[arg(long, env = "INCLUDE_SPOT", default_value_t = true,action = ArgAction::Set)]
    include_spot: bool,
    /// Include Binance USDⓈ-M Futures
    #[arg(long, env = "INCLUDE_USDM", default_value_t = true, action = ArgAction::Set)]
    include_usdm: bool,

    /// Dry-run: do not insert, just log the result count
    #[arg(long, default_value_t = false)]
    dry_run: bool,
}

#[derive(Deserialize)]
struct BnExchangeInfo { symbols: Vec<BnSymbol> }
#[derive(Deserialize)]
struct BnSymbol {
    symbol: String,
    status: String,
    baseAsset: String,
    quoteAsset: Option<String>, // spot has this; futures may not
}

#[derive(Deserialize, Debug, Clone)]
struct CgMarket {
    id: String,
    symbol: String,    // lowercase
    name: String,
    current_price: f64,
    market_cap: f64,
    total_volume: f64,
}

#[derive(Row, Serialize)]
struct McRow {
    ts: DateTime<Utc>,
    asset: String,
    cg_id: String,
    name: String,
    price_usd: f64,
    market_cap_usd: f64,
    volume_24h_usd: f64,
    available_spot: u8,
    available_usdm: u8,
    sources: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();
    let args = Args::parse();

    // 1) Collect assets present on Binance
    let (spot_set, usdm_set) = tokio::try_join!(
        async {
            if args.include_spot { fetch_binance_spot_assets().await } else { Ok(Default::default()) }
        },
        async {
            if args.include_usdm { fetch_binance_usdm_assets().await } else { Ok(Default::default()) }
        }
    )?;
    let union_assets: std::collections::HashSet<String> =
        spot_set.union(&usdm_set).cloned().collect();

    info!("assets: spot={} usdm={} union={}",
      spot_set.len(), usdm_set.len(), union_assets.len());

    if union_assets.is_empty() {
        warn!("No assets selected (check INCLUDE_SPOT/INCLUDE_USDM). Exiting.");
        return Ok(());
    }

    // 2) Pull CoinGecko market list (sorted by cap desc), stop when < threshold
    let cg_map = fetch_coingecko_markets_map(args.min_market_cap_usd).await?;
    info!("coingecko entries >= cap: {}", cg_map.len());

    // 3) Join & filter
    let now = Utc::now();
    let mut out_rows: Vec<McRow> = Vec::new();
    for asset in union_assets {
        let key = asset.to_lowercase();
        if let Some(best) = cg_map.get(&key) {
            if best.market_cap >= args.min_market_cap_usd {
                let row = McRow {
                    ts: now,
                    asset: asset.clone(),
                    cg_id: best.id.clone(),
                    name: best.name.clone(),
                    price_usd: best.current_price,
                    market_cap_usd: best.market_cap,
                    volume_24h_usd: best.total_volume,
                    available_spot: if args.include_spot && spot_set.contains(&asset) {1} else {0},
                    available_usdm: if args.include_usdm && usdm_set.contains(&asset) {1} else {0},
                    sources: "coingecko+binance".into(),
                };
                out_rows.push(row);
            }
        }
    }

    info!("snapshot rows to insert: {}", out_rows.len());
    if args.dry_run { return Ok(()); }

    // 4) Insert into ClickHouse
    let ch = build_ch(&args);
    insert_json_each_row(
        &args.ch_url,
        &args.ch_db,
        args.ch_user.as_deref(),
        args.ch_password.as_deref(),
        &out_rows,
    ).await?;
    info!("insert done");
    Ok(())
}

fn init_tracing() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into());
    tracing_subscriber::fmt().with_env_filter(filter).compact().init();
}

fn build_ch(a: &Args) -> Ch {
    let mut c = Ch::default().with_url(&a.ch_url).with_database(&a.ch_db);
    if let (Some(u), Some(p)) = (a.ch_user.as_ref(), a.ch_password.as_ref()) {
        c = c.with_user(u).with_password(p);
    }
    c
}

/// Binance SPOT: exchangeInfo → baseAsset (prefer items that trade against USDT)
async fn fetch_binance_spot_assets() -> Result<std::collections::HashSet<String>> {
    let info: BnExchangeInfo = reqwest::Client::new()
        .get("https://api.binance.com/api/v3/exchangeInfo")
        .send().await?.error_for_status()?.json().await?;
    let mut set = std::collections::HashSet::new();
    for s in info.symbols {
        if s.status == "TRADING" {
            // only consider symbols that have USDT quote to avoid fringe base assets
            if matches!(s.quoteAsset.as_deref(), Some("USDT")) {
                set.insert(s.baseAsset);
            }
        }
    }
    Ok(set)
}

/// Binance USDⓈ-M Futures: exchangeInfo → baseAsset (contracts in TRADING)
async fn fetch_binance_usdm_assets() -> Result<std::collections::HashSet<String>> {
    let info: BnExchangeInfo = reqwest::Client::new()
        .get("https://fapi.binance.com/fapi/v1/exchangeInfo")
        .send().await?.error_for_status()?.json().await?;
    let mut set = std::collections::HashSet::new();
    for s in info.symbols {
        if s.status == "TRADING" {
            set.insert(s.baseAsset);
        }
    }
    Ok(set)
}

/// Fetch CoinGecko /coins/markets pages until market_cap < threshold.
/// Keep best-by-marketcap per symbol (case-insensitive).
async fn fetch_coingecko_markets_map(min_cap: f64) -> Result<HashMap<String, CgMarket>> {
    let client = reqwest::Client::builder()
        .user_agent("mc-snapshot/0.1 (rust)")
        .build()?;
    let mut map: HashMap<String, CgMarket> = HashMap::new();
    let mut page = 1;
    loop {
        let mut resp = client.get("https://api.coingecko.com/api/v3/coins/markets")
            .query(&[
                ("vs_currency","usd"),
                ("order","market_cap_desc"),
                ("per_page","250"),
                ("page",&page.to_string()),
                ("price_change_percentage","24h"),
            ])
            .send().await?;
        // simple 429 backoff
        if resp.status() == StatusCode::TOO_MANY_REQUESTS {
            warn!("CoinGecko 429, backing off 2s...");
            sleep(Duration::from_secs(2)).await;
            continue;
        }
        resp.error_for_status_ref()?;
        let page_items: Vec<CgMarket> = resp.json().await?;
        if page_items.is_empty() { break; }
        let mut stop = false;

        for it in page_items {
            if it.market_cap < min_cap {
                stop = true; break;
            }
            let key = it.symbol.to_ascii_lowercase();
            // keep highest cap per ticker symbol
            map.entry(key.clone())
                .and_modify(|e| if it.market_cap > e.market_cap { *e = it.clone(); })
                .or_insert(it);
        }
        if stop { break; }
        page += 1;
        if page > 10 { // safety cap (~2500 coins)
            break;
        }
    }
    Ok(map)
}
async fn insert_json_each_row(
    ch_url: &str,
    db: &str,
    user: Option<&str>,
    pass: Option<&str>,
    rows: &[McRow],
) -> Result<()> {
    // Build NDJSON body
    let mut body = String::with_capacity(rows.len() * 256);
    for r in rows {
        // Safe format for DateTime64(3)
        let ts = r.ts.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        let line = serde_json::json!({
            "ts": ts,
            "asset": r.asset,
            "cg_id": r.cg_id,
            "name": r.name,
            "price_usd": r.price_usd,
            "market_cap_usd": r.market_cap_usd,
            "volume_24h_usd": r.volume_24h_usd,
            "available_spot": r.available_spot,
            "available_usdm": r.available_usdm,
            "sources": r.sources,
        });
        body.push_str(&line.to_string());
        body.push('\n'); // each row on its own line
    }

    let client = Client::new();
    // Put database + settings in query string; ClickHouse parses them as settings.
    // NOTE: keep query and settings as separate params — reqwest will handle encoding.
    let mut req = client
        .post(format!("{}/", ch_url))
        .query(&[
            ("database", db),
            ("date_time_input_format", "best_effort"),
            // optional but useful when schema evolves:
            ("input_format_skip_unknown_fields", "1"),
            ("query", "INSERT INTO assets_marketcap_snapshot FORMAT JSONEachRow"),
        ])
        .header("Content-Type", "application/x-ndjson")
        .body(body);

    if let Some(u) = user {
        req = req.basic_auth(u, pass);
    }

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();

    if !status.is_success() {
        // Surface server error details to logs
        bail!("ClickHouse HTTP {}: {}", status, text);
    }
    Ok(())
}
