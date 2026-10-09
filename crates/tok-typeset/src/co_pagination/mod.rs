//! Multi-Stream Co-Pagination & Active Synchronization Module.
//!
//! Provides production-grade layout mathematics, active anchor synchronization,
//! multi-page co-pagination with automatic overflow splitting, and non-blocking
//! background worker execution for Talmud, Mikraot Gedolot, and academic commentaries.

pub mod engine;
pub mod sync;
pub mod template;
pub mod worker;

pub use engine::{
    CoPaginatedChunk, CoPaginatedCommentary, CoPaginatedPage, CoPaginationDocument,
    CoPaginationEngine, CoPaginationResult,
};
pub use sync::{
    ActiveSyncResult, ActiveSynchronizer, AnchorKey, AnchorPoint, SyncRemedy, SynchronizerConfig,
};
pub use template::{ColumnAllocation, SpreadMathematics, TemplateConfig, TemplateKind};
pub use worker::{CancellationToken, NonBlockingPaginator, PaginationEvent};
