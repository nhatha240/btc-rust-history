use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use hft_proto::encode::from_bytes;
use hft_proto::md::FeatureState;
use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::Message;
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::str::FromStr;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

use zscore_model::{
    build_zscore_signal, FactorInput, RegimeFilterConfig, SignalBuildConfig,
    VolatilityFilterConfig, WeightMode,
};

mod zscore_model;

const EPSILON: f64 = 1e-12;

#[derive(Debug, Clone)]
struct RuntimeConfig {
    kafka_brokers: String,
    topic_in: String,
    topic_out: String,
    database_url: String,
    instance_id: String,
    strategy_name: String,
    model_version: String,
    signal_threshold: f64,
    history_capacity: usize,
    model_config: SignalBuildConfig,
    regime_filter_enabled: bool,
    allowed_regimes: Vec<String>,
    vol_filter_enabled: bool,
    vol_min_zscore: f64,
    vol_max_zscore: f64,
}

#[derive(Debug, Default, Clone)]
struct SymbolState {
    last_vwap: Option<f64>,
    pending_factors: Option<BTreeMap<String, f64>>,
    factor_histories: BTreeMap<String, VecDeque<f64>>,
    return_history: VecDeque<f64>,
    volatility_history: VecDeque<f64>,
}

#[derive(Debug)]
struct SignalRecord {
    signal_id: Uuid,
    trace_id: Uuid,
    event_ts: DateTime<Utc>,
    side_code: i32,
    side_label: &'static str,
    confidence: f64,
}

async fn write_strat_log(
    pool: &Pool<Postgres>,
    strategy_id: &str,
    symbol: &str,
    event_code: &str,
    message: &str,
    context: Option<serde_json::Value>,
) {
    let ctx = context.unwrap_or_else(|| serde_json::json!({}));
    let res = sqlx::query(
        r#"
        INSERT INTO strat_logs (strategy_version_id, symbol, log_level, event_code, message, context_json)
        VALUES ($1, $2, 'INFO', $3, $4, $5)
        "#,
    )
    .bind(strategy_id)
    .bind(symbol)
    .bind(event_code)
    .bind(message)
    .bind(ctx)
    .execute(pool)
    .await;

    if let Err(err) = res {
        error!(error=%err, "failed to write strat_log");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .json()
        .init();

    let cfg = RuntimeConfig::from_env()?;
    info!(
        topic_in=%cfg.topic_in,
        topic_out=%cfg.topic_out,
        strategy_name=%cfg.strategy_name,
        model_version=%cfg.model_version,
        "signal_engine starting"
    );

    let pg_pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&cfg.database_url)
        .await
        .context("failed to connect PostgreSQL")?;

    start_heartbeat_task(
        pg_pool.clone(),
        cfg.instance_id.clone(),
        cfg.strategy_name.clone(),
    );

    let producer: FutureProducer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("message.timeout.ms", "5000")
        .set("acks", "all")
        .create()
        .context("failed to create Kafka producer")?;

    let consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &cfg.kafka_brokers)
        .set("group.id", "signal-engine-group")
        .set("enable.partition.eof", "false")
        .set("session.timeout.ms", "6000")
        .set("enable.auto.commit", "true")
        .set("auto.offset.reset", "latest")
        .create()
        .context("failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&cfg.topic_in])
        .context("failed to subscribe feature topic")?;

    let mut states = HashMap::<String, SymbolState>::new();

    loop {
        match consumer.recv().await {
            Err(err) => error!(error=%err, "kafka receive error"),
            Ok(msg) => {
                let payload = match msg.payload_view::<[u8]>() {
                    Some(Ok(bytes)) => bytes,
                    _ => continue,
                };

                let feature: FeatureState = match from_bytes(payload) {
                    Ok(f) => f,
                    Err(err) => {
                        error!(error=%err, "failed to decode FeatureState");
                        continue;
                    }
                };

                if !feature.vwap.is_finite() || feature.vwap.abs() <= EPSILON {
                    warn!(symbol=%feature.symbol, vwap=feature.vwap, "skip feature due to invalid vwap");
                    continue;
                }

                let symbol = feature.symbol.clone();
                let event_ts = datetime_from_ms(feature.ts);
                let current_regime = regime_label(feature.regime);
                let current_volatility = current_volatility(&feature);
                let current_factors = extract_factor_snapshot(&feature);

                if current_factors.is_empty() {
                    warn!(symbol=%symbol, "skip feature because no finite factors extracted");
                    continue;
                }

                let state = states.entry(symbol.clone()).or_default();
                if let Some(vol) = current_volatility {
                    push_capped(&mut state.volatility_history, vol, cfg.history_capacity);
                }

                if let (Some(prev_vwap), Some(prev_factors)) =
                    (state.last_vwap, state.pending_factors.take())
                {
                    if prev_vwap.abs() > EPSILON {
                        let realized_return = (feature.vwap - prev_vwap) / prev_vwap;
                        if realized_return.is_finite() {
                            push_capped(
                                &mut state.return_history,
                                realized_return,
                                cfg.history_capacity,
                            );
                            for (factor_name, factor_value) in prev_factors {
                                let history =
                                    state.factor_histories.entry(factor_name).or_default();
                                push_capped(history, factor_value, cfg.history_capacity);
                            }
                        }
                    }
                }

                state.last_vwap = Some(feature.vwap);
                state.pending_factors = Some(current_factors.clone());

                let factor_inputs = build_factor_inputs(state, &current_factors, &cfg);
                if factor_inputs.is_empty() {
                    continue;
                }

                let mut model_cfg = cfg.model_config.clone();
                model_cfg.regime_filter = Some(RegimeFilterConfig {
                    enabled: cfg.regime_filter_enabled,
                    current_regime: Some(current_regime.clone()),
                    allowed_regimes: cfg.allowed_regimes.clone(),
                });
                model_cfg.volatility_filter = Some(VolatilityFilterConfig {
                    enabled: cfg.vol_filter_enabled,
                    historical_volatility_series: state
                        .volatility_history
                        .iter()
                        .copied()
                        .collect(),
                    current_volatility,
                    min_zscore: Some(cfg.vol_min_zscore),
                    max_zscore: Some(cfg.vol_max_zscore),
                });

                let signal_result = build_zscore_signal(&factor_inputs, &model_cfg);
                let record = build_signal_record(event_ts, &signal_result, cfg.signal_threshold);

                let metadata = serde_json::json!({
                    "feature_ts_ms": feature.ts,
                    "feature_schema_version": feature.schema_version,
                    "factor_snapshot": current_factors,
                    "regime_code": feature.regime,
                    "model": {
                        "rolling_window": model_cfg.rolling_window,
                        "min_history": model_cfg.min_history,
                        "min_abs_correlation": model_cfg.min_abs_correlation,
                        "weight_mode": format!("{:?}", model_cfg.weight_mode),
                    }
                });

                if let Err(err) = persist_zscore_signal(
                    &pg_pool,
                    &symbol,
                    &current_regime,
                    current_volatility,
                    &cfg.model_version,
                    &signal_result,
                    &record,
                    metadata,
                )
                .await
                {
                    error!(symbol=%symbol, error=%err, "failed to persist zscore signal");
                }

                let msg_text = format!(
                    "zscore signal={} side={} pred_return={}",
                    signal_result.signal, record.side_code, signal_result.predicted_return
                );
                write_strat_log(
                    &pg_pool,
                    &cfg.model_version,
                    &symbol,
                    "ZSCORE_SIGNAL",
                    &msg_text,
                    Some(serde_json::json!({
                        "signal_id": record.signal_id,
                        "trace_id": record.trace_id,
                        "signal": signal_result.signal,
                        "predicted_return": signal_result.predicted_return,
                        "suppressed": signal_result.suppressed,
                        "suppression_reasons": signal_result.suppression_reasons,
                    })),
                )
                .await;

                let kafka_message = serde_json::json!({
                    "signal_id": record.signal_id.to_string(),
                    "trace_id": record.trace_id.to_string(),
                    "symbol": symbol,
                    "ts": feature.ts,
                    "side": record.side_code,
                    "reason": "ZSCORE_MULTI_FACTOR",
                    "model_version": cfg.model_version,
                    "signal": signal_result.signal,
                    "predicted_return": signal_result.predicted_return,
                    "confidence": record.confidence,
                    "suppressed": signal_result.suppressed,
                    "suppression_reasons": signal_result.suppression_reasons,
                    "factors_used": signal_result.factors_used,
                    "total_factors": signal_result.total_factors
                });

                if let Ok(payload_json) = serde_json::to_string(&kafka_message) {
                    let _ = producer
                        .send(
                            FutureRecord::to(&cfg.topic_out)
                                .payload(&payload_json)
                                .key(&feature.symbol),
                            Duration::from_secs(0),
                        )
                        .await;
                }

                info!(
                    symbol=%feature.symbol,
                    signal=signal_result.signal,
                    predicted_return=signal_result.predicted_return,
                    side=record.side_code,
                    suppressed=signal_result.suppressed,
                    factors_used=signal_result.factors_used,
                    "zscore signal emitted"
                );
            }
        }
    }
}

impl RuntimeConfig {
    fn from_env() -> Result<Self> {
        let kafka_brokers = env_or("KAFKA_BROKERS", "redpanda:9092");
        let topic_in = env_or("KAFKA_TOPIC_FEATURE_STATE", "TOPIC_FEATURE_STATE");
        let topic_out = env_or("KAFKA_TOPIC_SIGNALS", "TOPIC_SIGNALS");
        let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let instance_id = env_or("SIGNAL_ENGINE_INSTANCE_ID", "signal_engine_01");
        let strategy_name = env_or("SIGNAL_ENGINE_STRATEGY_NAME", "zscore_multi_factor");
        let model_version = env_or("ZSCORE_MODEL_VERSION", "zscore-v1.0.0");
        let signal_threshold = env_parse("ZSCORE_SIGNAL_THRESHOLD", 0.2_f64)?;

        let rolling_window = env_parse("ZSCORE_ROLLING_WINDOW", 120_usize)?;
        let min_history = env_parse("ZSCORE_MIN_HISTORY", 30_usize)?;
        let min_abs_correlation = env_parse("ZSCORE_MIN_ABS_CORR", 0.03_f64)?;
        let rank_decay = env_parse("ZSCORE_RANK_DECAY", 0.85_f64)?;
        let weight_mode = WeightMode::from_env(&env_or("ZSCORE_WEIGHT_MODE", "equal"));
        let winsorize_zscore_at = env_parse("ZSCORE_WINSORIZE", 4.0_f64)?;

        let history_capacity = env_parse(
            "ZSCORE_HISTORY_CAPACITY",
            rolling_window.max(min_history).max(120) * 4,
        )?;

        let regime_filter_enabled = env_bool("ZSCORE_REGIME_FILTER_ENABLED", true);
        let allowed_regimes = env_or(
            "ZSCORE_ALLOWED_REGIMES",
            "TREND_UP,TREND_DOWN,RANGE,VOL_COMPRESSION,VOL_EXPANSION",
        )
        .split(',')
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

        let vol_filter_enabled = env_bool("ZSCORE_VOL_FILTER_ENABLED", true);
        let vol_min_zscore = env_parse("ZSCORE_VOL_MIN_Z", -2.5_f64)?;
        let vol_max_zscore = env_parse("ZSCORE_VOL_MAX_Z", 2.5_f64)?;

        let model_config = SignalBuildConfig {
            rolling_window,
            min_history,
            min_abs_correlation,
            rank_decay,
            weight_mode,
            winsorize_zscore_at,
            mean_return: None,
            std_return: None,
            regime_filter: None,
            volatility_filter: None,
        };

        Ok(Self {
            kafka_brokers,
            topic_in,
            topic_out,
            database_url,
            instance_id,
            strategy_name,
            model_version,
            signal_threshold,
            history_capacity,
            model_config,
            regime_filter_enabled,
            allowed_regimes,
            vol_filter_enabled,
            vol_min_zscore,
            vol_max_zscore,
        })
    }
}

fn env_or(key: &str, default_value: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default_value.to_string())
}

fn env_bool(key: &str, default_value: bool) -> bool {
    match std::env::var(key) {
        Ok(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default_value,
    }
}

fn env_parse<T>(key: &str, default_value: T) -> Result<T>
where
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(raw) => raw
            .parse::<T>()
            .map_err(|err| anyhow::anyhow!("invalid {key}={raw}: {err}")),
        Err(_) => Ok(default_value),
    }
}

fn start_heartbeat_task(pool: Pool<Postgres>, instance_id: String, strategy_name: String) {
    tokio::spawn(async move {
        loop {
            let res = sqlx::query(
                r#"
                INSERT INTO strat_health (instance_id, strategy_name, reported_at, cpu_pct, mem_mb)
                VALUES ($1, $2, now(), 0.0, 0.0)
                "#,
            )
            .bind(&instance_id)
            .bind(&strategy_name)
            .execute(&pool)
            .await;

            if let Err(err) = res {
                error!(error=%err, "failed to write heartbeat");
            }

            tokio::time::sleep(Duration::from_secs(30)).await;
        }
    });
}

fn push_capped<T>(queue: &mut VecDeque<T>, value: T, max_len: usize) {
    queue.push_back(value);
    while queue.len() > max_len {
        let _ = queue.pop_front();
    }
}

fn current_volatility(feature: &FeatureState) -> Option<f64> {
    if feature.atr.is_finite() && feature.vwap.is_finite() && feature.vwap.abs() > EPSILON {
        let value = (feature.atr / feature.vwap).abs();
        if value.is_finite() {
            return Some(value);
        }
    }

    if feature.vol_zscore.is_finite() {
        return Some(feature.vol_zscore.abs());
    }

    None
}

fn extract_factor_snapshot(feature: &FeatureState) -> BTreeMap<String, f64> {
    let mut factors = BTreeMap::<String, f64>::new();

    if feature.ema_slow.abs() > EPSILON
        && feature.ema_fast.is_finite()
        && feature.ema_slow.is_finite()
    {
        factors.insert(
            "ema_spread".to_string(),
            (feature.ema_fast - feature.ema_slow) / feature.ema_slow.abs(),
        );
    }

    if feature.macd_hist.is_finite() {
        factors.insert("macd_hist".to_string(), feature.macd_hist);
    }

    if feature.rsi.is_finite() {
        factors.insert("rsi_centered".to_string(), (feature.rsi - 50.0) / 50.0);
    }

    if feature.oi_change_pct.is_finite() {
        factors.insert("oi_change_pct".to_string(), feature.oi_change_pct);
    }

    if feature.adx.is_finite() {
        factors.insert("adx_centered".to_string(), (feature.adx - 25.0) / 25.0);
    }

    if feature.vol_zscore.is_finite() {
        factors.insert("vol_zscore".to_string(), feature.vol_zscore);
    }

    factors
}

fn build_factor_inputs(
    state: &SymbolState,
    current_factors: &BTreeMap<String, f64>,
    cfg: &RuntimeConfig,
) -> Vec<FactorInput> {
    let return_series = state.return_history.iter().copied().collect::<Vec<_>>();
    if return_series.len() < cfg.model_config.min_history {
        return vec![];
    }

    state
        .factor_histories
        .iter()
        .filter_map(|(factor_name, history)| {
            let current_value = current_factors.get(factor_name).copied();
            if current_value.is_none() {
                return None;
            }

            let series = history.iter().copied().collect::<Vec<_>>();
            if series.len() < cfg.model_config.min_history {
                return None;
            }

            Some(FactorInput {
                name: factor_name.clone(),
                series,
                return_series: return_series.clone(),
                weight: default_factor_weight(factor_name),
                rolling_window: Some(cfg.model_config.rolling_window),
                current_value,
                direction_hint: None,
                enabled: true,
            })
        })
        .collect()
}

fn default_factor_weight(name: &str) -> f64 {
    match name {
        "ema_spread" => 1.2,
        "macd_hist" => 1.1,
        "rsi_centered" => 1.0,
        "adx_centered" => 0.9,
        "oi_change_pct" => 0.8,
        "vol_zscore" => 0.7,
        _ => 1.0,
    }
}

fn regime_label(regime: u32) -> String {
    match regime {
        1 => "TREND_UP",
        2 => "TREND_DOWN",
        3 => "RANGE",
        4 => "VOL_COMPRESSION",
        5 => "VOL_EXPANSION",
        6 => "PANIC",
        7 => "ILLIQUID",
        8 => "HIGH_SPREAD_NO_TRADE",
        _ => "UNKNOWN",
    }
    .to_string()
}

fn datetime_from_ms(ts_ms: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ts_ms).unwrap_or_else(Utc::now)
}

fn build_signal_record(
    event_ts: DateTime<Utc>,
    result: &zscore_model::SignalResult,
    threshold: f64,
) -> SignalRecord {
    let (side_code, side_label) = if result.suppressed || result.signal.abs() < threshold {
        (0, "NEUTRAL")
    } else if result.signal > 0.0 {
        (1, "LONG")
    } else {
        (-1, "SHORT")
    };

    SignalRecord {
        signal_id: Uuid::new_v4(),
        trace_id: Uuid::new_v4(),
        event_ts,
        side_code,
        side_label,
        confidence: result.signal.abs().min(1.0),
    }
}

async fn persist_zscore_signal(
    pool: &Pool<Postgres>,
    symbol: &str,
    current_regime: &str,
    current_volatility: Option<f64>,
    model_version: &str,
    result: &zscore_model::SignalResult,
    record: &SignalRecord,
    metadata: serde_json::Value,
) -> Result<()> {
    let mut tx = pool
        .begin()
        .await
        .context("begin tx for zscore signal failed")?;

    sqlx::query(
        r#"
        INSERT INTO sig_zscore_signals (
            signal_id, trace_id, symbol, event_ts, side,
            signal_value, predicted_return, mean_return, std_return, confidence,
            suppressed, suppression_reasons, regime_passed, volatility_passed,
            factors_used, total_factors, model_version, current_regime,
            current_volatility, metadata
        ) VALUES (
            $1, $2, $3, $4, $5::signal_direction,
            $6, $7, $8, $9, $10,
            $11, $12, $13, $14,
            $15, $16, $17, $18,
            $19, $20
        )
        "#,
    )
    .bind(record.signal_id)
    .bind(record.trace_id)
    .bind(symbol)
    .bind(record.event_ts)
    .bind(record.side_label)
    .bind(result.signal)
    .bind(result.predicted_return)
    .bind(result.mean_return)
    .bind(result.std_return)
    .bind(record.confidence)
    .bind(result.suppressed)
    .bind(result.suppression_reasons.clone())
    .bind(result.regime_passed)
    .bind(result.volatility_passed)
    .bind(result.factors_used as i32)
    .bind(result.total_factors as i32)
    .bind(model_version)
    .bind(current_regime)
    .bind(current_volatility)
    .bind(metadata)
    .execute(&mut *tx)
    .await
    .context("insert sig_zscore_signals failed")?;

    for contrib in &result.contributions {
        sqlx::query(
            r#"
            INSERT INTO sig_zscore_factor_contribs (
                event_ts, signal_id, symbol, factor_name, factor_rank,
                correlation, sign, zscore, raw_weight, normalized_weight,
                contribution, passed, reject_reason
            ) VALUES (
                $1, $2, $3, $4, $5,
                $6, $7, $8, $9, $10,
                $11, $12, $13
            )
            "#,
        )
        .bind(record.event_ts)
        .bind(record.signal_id)
        .bind(symbol)
        .bind(&contrib.name)
        .bind(contrib.rank.map(|v| v as i32))
        .bind(contrib.correlation)
        .bind(contrib.sign as i16)
        .bind(contrib.zscore)
        .bind(contrib.raw_weight)
        .bind(contrib.normalized_weight)
        .bind(contrib.contribution)
        .bind(contrib.passed)
        .bind(contrib.reject_reason.clone())
        .execute(&mut *tx)
        .await
        .context("insert sig_zscore_factor_contribs failed")?;
    }

    tx.commit()
        .await
        .context("commit zscore signal tx failed")?;
    Ok(())
}
