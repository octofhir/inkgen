pub mod schema;
pub mod parser;

pub use schema::{InkgenConfig, PackageSpec, TreeShakingConfig, LanguageConfigs, TypeScriptConfig};
pub use parser::*;