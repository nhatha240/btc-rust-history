pub mod model;        // domain models (Model)
pub mod event;        // topics + message contracts (Module: transport)
pub mod indicators;   // technical indicators (Feature)
pub mod kafka;        // Kafka helpers (Service infra)
pub mod clickhouse;   // ClickHouse helpers (Service infra)

pub use model::*;
pub use event::*;
pub use indicators::*;
