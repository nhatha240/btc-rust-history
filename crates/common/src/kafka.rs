use anyhow::Result;
use rdkafka::{
    ClientConfig,
    producer::{FutureProducer, FutureRecord},
    consumer::{StreamConsumer, Consumer},
    util::Timeout
};

pub fn producer(brokers: &str) -> FutureProducer {
    ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("message.timeout.ms", "30000")
        .create()
        .expect("producer")
}

pub fn consumer(brokers: &str, group: &str, topics: &[&str]) -> Result<StreamConsumer> {
    let c: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .set("group.id", group)
        .set("enable.auto.commit", "false")
        .set("auto.offset.reset", "earliest")
        .create()?;
    c.subscribe(topics)?;
    Ok(c)
}

pub async fn send_json<P: serde::Serialize>(
    p: &FutureProducer, topic: &str, key: &str, payload: &P
) -> Result<()> {
    let bytes = serde_json::to_vec(payload)?;
    p.send(
        FutureRecord::to(topic).key(key).payload(&bytes),
        Timeout::Never
    ).await.expect("red kafka send error");
    Ok(())
}
