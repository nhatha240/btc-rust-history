pub mod encode;
pub mod generated;
pub mod versions;

// Re-export common modules for convenience
pub use generated::ai;
pub use generated::common;
pub use generated::md;
pub use generated::oms;

// Also re-export everything at the top level if needed,
// but keeping modules separate is cleaner.
pub use generated::oms::ExecutionReport;
pub use generated::oms::ExecutionStatus;
pub use generated::oms::OrderCommand;
