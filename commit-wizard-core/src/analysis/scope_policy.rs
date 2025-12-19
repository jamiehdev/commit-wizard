use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeStrategy {
    Auto,
    Conservative,
    Free,
}

impl Default for ScopeStrategy {
    fn default() -> Self {
        Self::Auto
    }
}
