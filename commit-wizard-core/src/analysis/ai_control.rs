use clap::ValueEnum;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AiControl {
    High,
    Balanced,
    Strict,
}

impl Default for AiControl {
    fn default() -> Self {
        Self::High
    }
}
