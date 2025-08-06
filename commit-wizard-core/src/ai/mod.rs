// ai module - organises ai-related functionality into submodules

pub mod api;
pub mod intelligence;
pub mod models;
pub mod patterns;
pub mod prompts;
pub mod validation;

// re-export key public items for convenient access
pub use api::{generate_conventional_commit, generate_conventional_commit_with_model};
pub use intelligence::{analyse_commit_intelligence, CommitIntelligence};
pub use models::{get_available_models, select_model_for_complexity};
pub use patterns::{Pattern, PatternType};
pub use validation::validate_commit_message;
