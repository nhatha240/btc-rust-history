//! `common` — shared proto types, domain models and utilities.
//!
//! Generated protobuf modules are included from `OUT_DIR` (via `build.rs`).
//! Import them in service code as:
//!
//! ```rust
//! use common::proto::md::FeatureState;
//! use common::proto::ai::AiPrediction;
//! use common::proto::common::Envelope;
//! ```

/// Auto-generated protobuf types compiled from `proto/**/*.proto`.
pub mod proto {
    pub mod common {
        include!(concat!(env!("OUT_DIR"), "/common.rs"));
    }
    pub mod md {
        include!(concat!(env!("OUT_DIR"), "/md.rs"));
    }
    pub mod ai {
        include!(concat!(env!("OUT_DIR"), "/ai.rs"));
    }
    pub mod oms {
        include!(concat!(env!("OUT_DIR"), "/oms.rs"));
    }
    pub mod control {
        include!(concat!(env!("OUT_DIR"), "/control.rs"));
    }
}
