use anyhow::{Context, Result};
use clickhouse::{Client, Row};
use serde::Deserialize;

const DEFAULT_HOST: &str = "http://127.0.0.1:8123";
const DEFAULT_USER: &str = "default";
const DEFAULT_PASSWORD: &str = "";
const DEFAULT_DB: &str = "db_trading";
const DEFAULT_CHUNK: usize = 5;

// You can override via env:
// CH_HOST, CH_USERNAME, CH_PASSWORD, CH_DATABASE,
// SRC_TABLE, DST_TABLE, CHUNK_SIZE

#[derive(Debug, Row, Deserialize)]
struct SymRow {
    symbol: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let host = std::env::var("CH_HOST").unwrap_or_else(|_| DEFAULT_HOST.into());
    let username = std::env::var("CH_USERNAME").unwrap_or_else(|_| DEFAULT_USER.into());
    let database = std::env::var("CH_DATABASE").unwrap_or_else(|_| DEFAULT_DB.into());

    let src_table = std::env::var("SRC_TABLE")
        .unwrap_or_else(|_| "db_trading.candles_1m_final".into());
    let dst_table = std::env::var("DST_TABLE")
        .unwrap_or_else(|_| "db_trading.candles_4h".into());
    let chunk_size = std::env::var("CHUNK_SIZE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(DEFAULT_CHUNK);

    let client = Client::default()
        .with_url(&host)
        .with_user(&username)
        .with_database(&database);

    println!(
        "[agg_15m] connecting to ClickHouse: {} db={} src={} dst={} chunk={}",
        host, database, src_table, dst_table, chunk_size
    );

    let symbols = get_symbols(&client, &src_table).await?;
    println!("[agg_15m] symbols: {} found", symbols.len());
    if symbols.is_empty() {
        println!("[agg_15m] nothing to do.");
        return Ok(());
    }

    for batch in symbols.chunks(chunk_size) {
        // insert_agg_for_symbols(&client, &src_table, "candles_15m".parse()?, batch).await?;
        // insert_agg_for_symbols(&client, &src_table, "candles_1h".parse()?, batch).await?;
        // insert_agg_for_symbols(&client, &src_table, "candles_4h".parse()?, batch).await?;
        insert_agg_for_symbols(&client, &src_table, "candles_1d", batch).await?;
        // insert_agg_for_symbols(&client, &src_table, "candles_1w", batch).await?;
        println!(
            "[agg_15m] inserted batch ({}): {}",
            batch.len(),
            preview_list(batch, 8)
        );
    }

    println!("[agg_15m] Done.");
    Ok(())
}

async fn get_symbols(client: &Client, src_table: &str) -> Result<Vec<String>> {
    let q = format!(
        "SELECT DISTINCT symbol FROM {} ORDER BY symbol",
        src_table
    );
    let rows: Vec<SymRow> = client
        .query(q.as_str())
        .fetch_all()
        .await
        .context("fetch symbols")?;
    Ok(rows.into_iter().map(|r| r.symbol).collect())
}

async fn insert_agg_for_symbols(
    client: &Client,
    src_table: &str,
    dst_table: &str,
    symbols_batch: &[String],
) -> Result<()> {
    // Build safe IN (...) list; symbols originate from our DB but we still escape single quotes.
    let in_clause = symbols_batch
        .iter()
        .map(|s| format!("'{}'", s.replace('\'', "\\'")))
        .collect::<Vec<_>>()
        .join(", ");

    let q = format!(
        r#"
        INSERT INTO {dst}
        SELECT
          toStartOfInterval(open_time, INTERVAL 4 HOUR) AS open_time,
          symbol,
          argMin(open, open_time)   AS open,
          max(high)                 AS high,
          min(low)                  AS low,
          argMax(close, open_time)  AS close,
          sum(volume)               AS volume,
          max(close_time)           AS close_time,
          sum(quote_asset_volume)   AS quote_asset_volume,
          sum(number_of_trades)     AS number_of_trades,
          sum(taker_buy_base_asset_volume)  AS taker_buy_base_asset_volume,
          sum(taker_buy_quote_asset_volume) AS taker_buy_quote_asset_volume
        FROM {src}
        WHERE symbol IN ({in_clause})
        GROUP BY symbol, open_time
        ORDER BY symbol, open_time
        "#,
        dst = dst_table,
        src = src_table,
        in_clause = in_clause
    );

    client.query(q.as_str()).execute().await.context("insert agg")
}

fn preview_list(list: &[String], max_items: usize) -> String {
    if list.len() <= max_items {
        return list.join(", ");
    }
    let head = &list[..max_items];
    format!("{}, ... (+{} more)", head.join(", "), list.len() - max_items)
}
