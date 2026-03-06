//! Registry of per-symbol indicator state backed by [`dashmap::DashMap`].
//!
//! `DashMap` provides lock-free concurrent reads and fine-grained shard
//! locking on writes, keeping memory growth strictly proportional to the
//! number of active symbols.

use dashmap::DashMap;

use crate::config::Config;
use crate::state::symbol_state::SymbolState;

/// Thread-safe registry mapping `symbol → SymbolState`.
pub struct Registry {
    map: DashMap<String, SymbolState>,
    cfg: Config,
}

impl Registry {
    pub fn new(cfg: Config) -> Self {
        Self {
            map: DashMap::new(),
            cfg,
        }
    }

    /// Return a mutable reference to the per-symbol state, creating it on
    /// first access.
    pub fn get_or_create(&self, symbol: &str) -> dashmap::mapref::one::RefMut<'_, String, SymbolState> {
        if !self.map.contains_key(symbol) {
            let state = SymbolState::new(
                symbol,
                self.cfg.ema_fast_period,
                self.cfg.ema_slow_period,
                self.cfg.rsi_period,
                self.cfg.macd_signal_period,
            );
            self.map.insert(symbol.to_owned(), state);
        }
        self.map.get_mut(symbol).expect("just inserted")
    }

    /// Number of symbols currently tracked.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }
}
