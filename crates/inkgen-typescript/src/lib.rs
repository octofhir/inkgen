//! TypeScript code generator backend for Inkgen

pub mod generator;
pub mod templates;

pub use generator::{TypeScriptGenerator, LanguageGenerator};