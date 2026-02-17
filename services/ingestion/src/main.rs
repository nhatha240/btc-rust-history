mod config;
mod models;

mod sources {
    pub mod binance_ws;
    pub mod binance_discovery;
}

use anyhow::Result;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Chọn crypto provider cho rustls 0.23
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install rustls provider");

    let cfg = config::Config::from_env()?;
    info!(?cfg, "ingestion start");

    let producer = common::kafka::producer(&cfg.kafka_brokers);

    // optional ClickHouse client
    let ch_client = match (&cfg.ch_url, &cfg.ch_db) {
        (Some(url), Some(db)) => Some(common::clickhouse::client(
            url, db, cfg.ch_user.as_deref(), cfg.ch_pass.as_deref(),
        )),
        _ => None,
    };

    // symbols
    let mut symbols = cfg.symbols.clone();
    if symbols.len() == 1 && symbols[0] == "*" {
        symbols = sources::binance_discovery::symbols_all(cfg.only_spot, cfg.quotes.clone()).await?;
        info!(count = symbols.len(), "discovered symbols");
    }

    // spawn combined streams theo vector chunks
    let mut tasks = Vec::new();
    for chunk in symbols.chunks(cfg.ws_chunk) {
        let p = producer.clone();
        let topic = cfg.kafka_topic.clone();
        let interval = cfg.interval.clone();
        let exchange = cfg.exchange.clone();
        let exchange_ws = exchange.clone();      // dùng cho stream_combined
        let chunk_vec: Vec<String> = chunk.to_vec();
        let ch_client2 = ch_client.clone();
        let ch_table2 = cfg.ch_table.clone();

        let fut = tokio::spawn(async move {
            let mut publish = {
                let p = p.clone();
                let topic = topic.clone();
                let ch_client2 = ch_client2.clone();
                let ch_table2 = ch_table2.clone();
                let exchange_cl = exchange.clone();

                move |evt: common::event::CandleEvent| {
                    // Kafka
                    let key = common::event::key_symbol(&exchange_cl, &evt.payload.symbol);
                    let p2 = p.clone();
                    let topic2 = topic.clone();
                    let evt2 = evt.clone();
                    tokio::spawn(async move {
                        let _ = common::kafka::send_json(&p2, &topic2, &key, &evt2).await;
                    });

                    // ClickHouse (tùy chọn)
                    if let (Some(ch), Some(tbl)) = (&ch_client2, &ch_table2) {
                        let candle = evt.payload.clone();
                        let ch2 = ch.clone();
                        let tbl2 = tbl.clone();
                        tokio::spawn(async move {
                            let _ = common::clickhouse::insert_candle_1m_final(&ch2, &candle).await;
                        });
                    }
                }
            };

            if let Err(e) = crate::sources::binance_ws::stream_combined(
                &exchange_ws, &chunk_vec, &interval, &mut publish,
            ).await {
                error!(size = chunk_vec.len(), error=%e, "combined stream stopped");
            }
        });
        tasks.push(fut);
    }

    tokio::signal::ctrl_c().await?;
    for t in tasks { t.abort(); }
    Ok(())
}
