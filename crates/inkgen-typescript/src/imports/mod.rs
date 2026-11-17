/// Import tracking and optimization system
///
/// This module provides tools for tracking type usage and optimizing imports
/// to reduce bundle size and enable better tree-shaking.
pub mod resolver;
/// Type usage tracking for tree-shaking and type-only import separation.
pub mod tracker;

pub use resolver::{TypeRegistry, calculate_import_path};
pub use tracker::{UsageContext, UsageTracker};
