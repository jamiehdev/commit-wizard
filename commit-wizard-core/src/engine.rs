use anyhow::{Context, Result};

use crate::ai;
use crate::git::{self, DiffInfo};
use crate::{safe_fallback_model, save_config, Config, CoreCliArgs};

pub struct CommitGenerationResult {
    pub diff_info: DiffInfo,
    pub commit_message: String,
    pub selected_model: String,
    pub used_fallback: bool,
    // audit fields from classifier (only populated when use_classifier=true)
    pub scope_reason: Option<String>,
    pub scope_confidence: Option<f32>,
    pub overall_confidence: Option<f32>,
}

pub async fn generate_commit_message(
    args: &CoreCliArgs,
    config: &mut Config,
    quiet: bool,
    use_classifier: bool,
) -> Result<CommitGenerationResult> {
    let repo_path = args.path.clone().unwrap_or_else(|| ".".to_string());

    let repo = match git2::Repository::discover(&repo_path) {
        Ok(repo) => repo,
        Err(e) => return Err(anyhow::anyhow!("invalid git repository: {}", e)),
    };

    if repo.is_bare() {
        return Err(anyhow::anyhow!("bare repositories not supported"));
    }

    let diff_info = git::get_diff_info(
        &repo_path,
        args.max_size * 1024,
        args.max_files,
        args.verbose && !quiet,
    )
    .context("failed to get git diff information")?;

    if diff_info.files.is_empty() {
        return Err(anyhow::anyhow!("no changes detected in the repository"));
    }

    let mut selected_model = crate::get_current_model(config, args, Some(&diff_info));
    let mut used_fallback = false;

    let min_confidence = args.effective_min_confidence(config);
    let min_scope_confidence = args.effective_min_scope_confidence(config);
    let scope_strategy = args.effective_scope_strategy(config);
    let scope_aliases = args.effective_scope_aliases(config);
    let ai_control = args.effective_ai_control(config);

    let gen_result = match ai::generate_conventional_commit_with_model(
        &diff_info,
        args.debug,
        args.smart_model,
        Some(selected_model.clone()),
        config,
        quiet,
        use_classifier,
        min_confidence,
        min_scope_confidence,
        scope_strategy,
        scope_aliases,
        ai_control,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("invalid model") {
                let fallback_model = safe_fallback_model(config);
                selected_model = fallback_model.clone();
                used_fallback = true;

                config.current_model = Some(fallback_model.clone());
                config.auto_select = false;
                if let Err(save_err) = save_config(config) {
                    return Err(save_err.context("failed to save recovery config"));
                }

                ai::generate_conventional_commit_with_model(
                    &diff_info,
                    args.debug,
                    args.smart_model,
                    Some(selected_model.clone()),
                    config,
                    quiet,
                    use_classifier,
                    min_confidence,
                    min_scope_confidence,
                    scope_strategy,
                    scope_aliases,
                    ai_control,
                )
                .await
                .context("failed to generate commit message even with fallback model")?
            } else {
                return Err(e.context("failed to generate commit message"));
            }
        }
    };

    Ok(CommitGenerationResult {
        diff_info,
        commit_message: gen_result.message,
        selected_model,
        used_fallback,
        scope_reason: gen_result.scope_reason,
        scope_confidence: gen_result.scope_confidence,
        overall_confidence: gen_result.overall_confidence,
    })
}
