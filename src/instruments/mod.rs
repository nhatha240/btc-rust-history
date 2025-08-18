// comment: Instruments (Spot/UM/CM) fetch + cache (Redis) + ClickHouse persist
use anyhow::{Context, Result};
use clickhouse::Row;
use rmp_serde::{encode::to_vec_named, decode::from_slice};
use serde::{Deserialize, Serialize};
use std::{time::{Duration, SystemTime, UNIX_EPOCH}};
use futures::AsyncWriteExt;
use tokio::{task::JoinSet, time::sleep};
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub struct InstrumentsCfg {
    pub spot_base: String,
    pub um_base: String,
    pub cm_base: String,

    pub redis_url: String,
    pub cache_ttl_sec: usize,
    pub ch_url: String,
    pub ch_db: String,

    pub only_trading: bool,
    pub allowed_quotes: Vec<String>,

    pub save_spot: bool,
    pub save_um: bool,
    pub save_cm: bool,

    pub redis_key_spot: String,
    pub redis_key_um: String,
    pub redis_key_cm: String,

    /// run sync every `period_sec`
    pub period_sec: u64,
}

impl InstrumentsCfg {
    pub fn from_env() -> Self {
        use std::env;
        let get = |k: &str, d: &str| env::var(k).unwrap_or_else(|_| d.to_string());
        let b = |k: &str, d: bool| env::var(k).ok().map(|v| v=="1"||v.eq_ignore_ascii_case("true")).unwrap_or(d);
        let u = |k: &str, d: usize| env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
        let list = |k: &str| -> Vec<String> {
            env::var(k).ok().map(|s| s.split(',').map(|x| x.trim().to_uppercase()).filter(|x|!x.is_empty()).collect()).unwrap_or_default()
        };
        Self {
            spot_base: get("BINANCE_SPOT_BASE", "https://api.binance.com"),
            um_base:   get("BINANCE_UM_BASE",   "https://fapi.binance.com"),
            cm_base:   get("BINANCE_CM_BASE",   "https://dapi.binance.com"),

            redis_url: get("REDIS_URL", "redis://127.0.0.1/"),
            cache_ttl_sec: u("CACHE_TTL_SEC", 3600),
            ch_url: get("CLICKHOUSE_URL", "http://127.0.0.1:8123"),
            ch_db:  get("CLICKHOUSE_DB",  "db_trading"),

            only_trading: b("ONLY_TRADING", true),
            allowed_quotes: list("ALLOWED_QUOTES"),

            save_spot: b("SAVE_SPOT", true),
            save_um:   b("SAVE_UM", true),
            save_cm:   b("SAVE_CM", true),

            redis_key_spot: get("REDIS_KEY_SPOT", "instruments:binance:spot"),
            redis_key_um:   get("REDIS_KEY_UM",   "instruments:binance:um"),
            redis_key_cm:   get("REDIS_KEY_CM",   "instruments:binance:cm"),

            period_sec: env::var("INSTR_SYNC_SEC").ok().and_then(|v| v.parse().ok()).unwrap_or(900),
        }
    }
}

// ---------- models ----------
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InstrumentRow {
    pub exchange: String,   // owned
    pub market:   String,   // owned
    pub symbol: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub status: String,
    pub contract_type: Option<String>,
    pub delivery_date_ms: Option<u64>,
    pub onboard_date_ms: Option<u64>,
    pub margin_asset: Option<String>,
    pub price_precision: Option<u32>,
    pub quantity_precision: Option<u32>,
    pub permissions_json: Option<String>,
    pub filters_json: Option<String>,
    pub version: u64,
}

#[derive(Serialize, Row)]
struct ChInstrumentRow<'a> {
    exchange: &'a str,
    market: &'a str,
    symbol: &'a str,
    base_asset: &'a str,
    quote_asset: &'a str,
    status: &'a str,
    contract_type: Option<&'a str>,
    delivery_date_ms: Option<i64>,
    onboard_date_ms: Option<i64>,
    margin_asset: Option<&'a str>,
    price_precision: Option<u32>,
    quantity_precision: Option<u32>,
    permissions_json: Option<&'a str>,
    filters_json: Option<&'a str>,
    version: u64,
}

// spot
#[derive(Deserialize)]
struct SpotExchangeInfo { symbols: Vec<SpotSymbol> }
#[derive(Deserialize)]
struct SpotSymbol {
    symbol: String, status: String, baseAsset: String, quoteAsset: String,
    permissions: Option<Vec<String>>,
    #[serde(default)] filters: Vec<serde_json::Value>,
    baseAssetPrecision: Option<u32>, quotePrecision: Option<u32>, quoteAssetPrecision: Option<u32>,
    isSpotTradingAllowed: Option<bool>,
}
// futures
#[derive(Deserialize)]
struct FutExchangeInfo { symbols: Vec<FutSymbol> }
#[derive(Deserialize)]
struct FutSymbol {
    symbol: String, status: String, contractType: String,
    baseAsset: String, quoteAsset: String, marginAsset: Option<String>,
    deliveryDate: Option<u64>, onboardDate: Option<u64>,
    pricePrecision: Option<u32>, quantityPrecision: Option<u32>,
    #[serde(default)] filters: Vec<serde_json::Value>,
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64
}

fn quote_allowed(cfg: &InstrumentsCfg, quote: &str) -> bool {
    cfg.allowed_quotes.is_empty() || cfg.allowed_quotes.iter().any(|q| q.eq_ignore_ascii_case(quote))
}

// ---------- fetchers ----------
pub async fn fetch_spot(cfg: &InstrumentsCfg) -> Result<Vec<InstrumentRow>> {
    let url = format!("{}/api/v3/exchangeInfo", cfg.spot_base);
    let cli = reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
    let data: SpotExchangeInfo = cli.get(url).send().await?.error_for_status()?.json().await?;
    let ts = now_ms();
    let mut out = Vec::with_capacity(data.symbols.len());
    for s in data.symbols {
        if cfg.only_trading && !s.status.eq_ignore_ascii_case("TRADING") { continue; }
        if !quote_allowed(cfg, &s.quoteAsset) { continue; }
        let filters_json = if s.filters.is_empty() { None } else { Some(serde_json::to_string(&s.filters)?) };
        let permissions_json = s.permissions.as_ref().map(|p| serde_json::to_string(p)).transpose()?;
        out.push(InstrumentRow {
            exchange: "binance".to_string(),
            market:   "spot".to_string(),
            symbol: s.symbol,
            base_asset: s.baseAsset,
            quote_asset: s.quoteAsset,
            status: s.status,
            contract_type: None,
            delivery_date_ms: None,
            onboard_date_ms: None,
            margin_asset: None,
            price_precision: s.quotePrecision.or(s.quoteAssetPrecision),
            quantity_precision: s.baseAssetPrecision,
            permissions_json,
            filters_json,
            version: ts,
        });

    }
    Ok(out)
}
pub async fn fetch_futures(cfg: &InstrumentsCfg, market: &'static str) -> Result<Vec<InstrumentRow>> {
    let base = if market=="um" { &cfg.um_base } else { &cfg.cm_base };
    let path = if market=="um" { "/fapi/v1/exchangeInfo" } else { "/dapi/v1/exchangeInfo" };
    let url = format!("{base}{path}");
    let cli = reqwest::Client::builder().timeout(Duration::from_secs(20)).build()?;
    let data: FutExchangeInfo = cli.get(url).send().await?.error_for_status()?.json().await?;
    let ts = now_ms();
    let mut out = Vec::with_capacity(data.symbols.len());
    for s in data.symbols {
        if cfg.only_trading && !s.status.eq_ignore_ascii_case("TRADING") { continue; }
        if !quote_allowed(cfg, &s.quoteAsset) { continue; }
        let filters_json = if s.filters.is_empty() { None } else { Some(serde_json::to_string(&s.filters)?) };
        out.push(InstrumentRow {
            exchange: "binance".to_string(),
            market:   market.to_string(),
            symbol: s.symbol,
            base_asset: s.baseAsset,
            quote_asset: s.quoteAsset,
            status: s.status,
            contract_type: Some(s.contractType),
            delivery_date_ms: s.deliveryDate.filter(|&x| x != 0),
            onboard_date_ms: s.onboardDate.filter(|&x| x != 0),
            margin_asset: s.marginAsset,
            price_precision: s.pricePrecision,
            quantity_precision: s.quantityPrecision,
            permissions_json: None,
            filters_json,
            version: ts,
        });
    }
    Ok(out)
}

// ---------- redis ----------
pub async fn cache_set(redis_url:&str, key:&str, ttl:usize, rows:&[InstrumentRow]) -> Result<()> {
    use redis::AsyncCommands;
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_async_connection().await?;
    let bytes = to_vec_named(rows)?;
    redis::cmd("SET").arg(key).arg(bytes).arg("EX").arg(ttl).query_async(&mut conn).await?;
    Ok(())
}

pub async fn cache_get(redis_url:&str, key:&str) -> Result<Option<Vec<InstrumentRow>>> {
    use redis::AsyncCommands;
    let client = redis::Client::open(redis_url)?;
    let mut conn = client.get_async_connection().await?;
    let opt: Option<Vec<u8>> = conn.get(key).await?;
    if let Some(b) = opt {
        let v: Vec<InstrumentRow> = rmp_serde::from_slice(&b)?;
        Ok(Some(v))
    } else { Ok(None) }
}

// ---------- clickhouse ----------
pub async fn ensure_clickhouse_schema(ch_url:&str, ch_db:&str) -> Result<()> {
    let client = clickhouse::Client::default().with_url(ch_url);
    client.query(format!("CREATE DATABASE IF NOT EXISTS {}", ch_db).as_str()).execute().await?;
    let client = client.with_database(ch_db);
    let ddl = r#"
CREATE TABLE IF NOT EXISTS instruments_all
(
  exchange LowCardinality(String),
  market LowCardinality(String),
  symbol String,
  base_asset LowCardinality(String),
  quote_asset LowCardinality(String),
  status LowCardinality(String),
  contract_type LowCardinality(Nullable(String)),
  delivery_date_ms Nullable(Int64),
  onboard_date_ms Nullable(Int64),
  margin_asset LowCardinality(Nullable(String)),
  price_precision Nullable(UInt32),
  quantity_precision Nullable(UInt32),
  permissions_json Nullable(String),
  filters_json Nullable(String),
  version UInt64
)
ENGINE = ReplacingMergeTree(version)
PARTITION BY market
ORDER BY (exchange, market, symbol)
SETTINGS allow_nullable_key = 1
"#;
    client.query(ddl).execute().await?;
    Ok(())
}

pub async fn save_clickhouse(rows: &[InstrumentRow], ch_url: &str, ch_db: &str) -> anyhow::Result<()> {
    let client = clickhouse::Client::default().with_url(ch_url).with_database(ch_db);
    const CHUNK: usize = 2000;
    for chunk in rows.chunks(CHUNK) {
        let mut insert = client.insert("instruments_all")?;
        for r in chunk {
            let row = ChInstrumentRow {
                exchange: r.exchange.as_str(),
                market:   r.market.as_str(),
                symbol:   &r.symbol,
                base_asset: &r.base_asset,
                quote_asset: &r.quote_asset,
                status: &r.status,
                contract_type: r.contract_type.as_deref(),
                delivery_date_ms: r.delivery_date_ms.map(|x| x as i64),
                onboard_date_ms:  r.onboard_date_ms.map(|x| x as i64),
                margin_asset: r.margin_asset.as_deref(),
                price_precision: r.price_precision,
                quantity_precision: r.quantity_precision,
                permissions_json: r.permissions_json.as_deref(),
                filters_json: r.filters_json.as_deref(),
                version: r.version,
            };
            insert.write(&row).await?;
        }
        insert.end().await?;
    }
    Ok(())
}


// ---------- one-shot + periodic ----------
pub async fn sync_once(cfg:&InstrumentsCfg) -> Result<usize> {
    let mut jobs = JoinSet::new();
    if cfg.save_spot { let c=cfg.clone(); jobs.spawn(async move { fetch_spot(&c).await.map(|v|("spot",v)) }); }
    if cfg.save_um   { let c=cfg.clone(); jobs.spawn(async move { fetch_futures(&c,"um").await.map(|v|("um",v))   }); }
    if cfg.save_cm   { let c=cfg.clone(); jobs.spawn(async move { fetch_futures(&c,"cm").await.map(|v|("cm",v))   }); }

    let mut total = 0usize;
    while let Some(res) = jobs.join_next().await {
        match res {
            Ok(Ok((market, rows))) => {
                info!("Fetched {market} instruments: {}", rows.len());
                if rows.is_empty() { continue; }
                let key = match market { "spot"=>&cfg.redis_key_spot, "um"=>&cfg.redis_key_um, "cm"=>&cfg.redis_key_cm, _=>unreachable!() };
                cache_set(&cfg.redis_url, key, cfg.cache_ttl_sec, &rows).await
                    .with_context(|| format!("redis save {market}"))?;
                save_clickhouse(&rows, &cfg.ch_url, &cfg.ch_db).await
                    .with_context(|| format!("clickhouse save {market}"))?;
                total += rows.len();
            }
            Ok(Err(e)) => { warn!("fetch error: {e:?}"); }
            Err(e) => { warn!("task join error: {e:?}"); }
        }
    }
    Ok(total)
}

pub async fn spawn_periodic_sync(cfg:InstrumentsCfg) {
    // ensure schema once
    if let Err(e) = ensure_clickhouse_schema(&cfg.ch_url, &cfg.ch_db).await {
        warn!("ensure CH schema failed: {e:?}");
    }
    loop {
        match sync_once(&cfg).await {
            Ok(n) => info!("Instruments sync OK, rows={n}"),
            Err(e) => warn!("Instruments sync error: {e:?}"),
        }
        sleep(Duration::from_secs(cfg.period_sec)).await;
    }
}
