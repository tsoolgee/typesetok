//! Multi-Stream Co-Pagination & Active Synchronization Module.
//!
//! Provides production-grade layout mathematics, active anchor synchronization,
//! multi-page co-pagination with automatic overflow splitting, and non-blocking
//! background worker execution for Talmud, Mikraot Gedolot, and academic commentaries.

pub mod sync;
pub mod template;

pub use sync::{
    ActiveSyncResult, ActiveSynchronizer, AnchorKey, AnchorPoint, SyncRemedy, SynchronizerConfig,
};
pub use template::{ColumnAllocation, SpreadMathematics, TemplateConfig, TemplateKind};
