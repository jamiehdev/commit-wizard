// model selection and management module

use super::intelligence::CommitIntelligence;
use crate::Config;

/// select model based on complexity
pub fn select_model_for_complexity(
    intelligence: &CommitIntelligence,
    debug: bool,
    config: &Config,
) -> String {
    let (model, reason) = if intelligence.complexity_score < 1.5 {
        (
            &config.models.fast,
            "simple commit detected - using fast model",
        )
    } else if intelligence.complexity_score < 2.5 {
        (
            &config.models.thinking,
            "medium complexity commit - using thinking model",
        )
    } else {
        (
            &config.models.thinking,
            "complex commit detected - using thinking model",
        )
    };

    if debug {
        println!("🤖 smart model selection: {reason} ({model})");
    }

    model.to_string()
}

/// get available models for manual selection
pub fn get_available_models(config: &Config) -> Vec<(&str, &str)> {
    config
        .models
        .available
        .iter()
        .map(|model| (model.name.as_str(), model.description.as_str()))
        .collect()
}
