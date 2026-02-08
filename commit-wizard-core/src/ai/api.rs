// api interaction module - handles openrouter api communication

use crate::git::DiffInfo;
use crate::Config;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

use super::classifier::{parse_commit_classification, render_commit_message_from_classification};
use super::intelligence::{analyse_commit_intelligence, CommitIntelligence};
use super::models::select_model_for_complexity;
use super::patterns::PatternType;
use super::prompts::{
    construct_classifier_prompt, construct_intelligent_prompt, extract_meaningful_diff_lines,
    get_system_prompt,
};
use super::validation::{
    extract_commit_message, fix_commit_format, post_process_commit_message, validate_commit_message,
};
use crate::analysis::{
    analyse_guardrails, calculate_scope_coverage_with_rules, derive_scope_candidates_with_rules,
    load_scope_rules, AiControl, ScopeCandidateOptions, ScopeStrategy,
};

// openrouter api structures
#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// result from commit generation with optional classification metadata
#[derive(Debug, Clone)]
pub struct GenerationResult {
    pub message: String,
    /// scope explanation from classifier (only when use_classifier=true)
    pub scope_reason: Option<String>,
    /// scope confidence 0.0-1.0 (only when use_classifier=true)
    pub scope_confidence: Option<f32>,
    /// overall confidence 0.0-1.0 (only when use_classifier=true)
    pub overall_confidence: Option<f32>,
}


/// generate a conventional commit message based on the diff information
pub async fn generate_conventional_commit(
    diff_info: &DiffInfo,
    debug: bool,
    smart_model: bool,
    quiet: bool,
    use_classifier: bool,
    min_confidence: Option<f32>,
    min_scope_confidence: Option<f32>,
    scope_strategy: crate::analysis::ScopeStrategy,
    scope_aliases: bool,
    ai_control: AiControl,
    config: &Config,
) -> Result<String> {
    generate_conventional_commit_with_model(
        diff_info,
        debug,
        smart_model,
        None,
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
    .map(|r| r.message)
}

/// generate a conventional commit message with optional custom model
pub async fn generate_conventional_commit_with_model(
    diff_info: &DiffInfo,
    debug: bool,
    smart_model: bool,
    custom_model: Option<String>,
    config: &Config,
    quiet: bool,
    use_classifier: bool,
    min_confidence: Option<f32>,
    min_scope_confidence: Option<f32>,
    scope_strategy: crate::analysis::ScopeStrategy,
    scope_aliases: bool,
    ai_control: AiControl,
) -> Result<GenerationResult> {
    let is_debug = debug;
    let spinner = if quiet {
        None
    } else {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                .template("{spinner:.blue} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        spinner.set_message("🧙 analysing commit changes...");
        spinner.enable_steady_tick(Duration::from_millis(80));
        Some(spinner)
    };

    // analyse commit intelligence (this is the expensive operation)
    let intelligence = analyse_commit_intelligence(diff_info);

    // select model based on complexity or custom choice
    let model = if let Some(custom) = custom_model {
        custom
    } else if smart_model {
        select_model_for_complexity(&intelligence, is_debug, config)
    } else {
        config.models.default.clone()
    };

    if is_debug {
        println!("🤖 selected model:\n{model}");
        println!();
    }

    if let Some(spinner) = &spinner {
        spinner.set_message(format!("🧙 generating commit message with {model}..."));
    }

    let guardrails = analyse_guardrails(diff_info);
    let scope_rules = load_scope_rules(&diff_info.repo_root);

    let effective_scope_strategy = match ai_control {
        AiControl::High => ScopeStrategy::Free,
        AiControl::Balanced => scope_strategy,
        AiControl::Strict => ScopeStrategy::Conservative,
    };

    let (scope_candidates, forced_type) = if use_classifier {
        let candidates = derive_scope_candidates_with_rules(
            diff_info,
            ScopeCandidateOptions {
                allow_aliases: scope_aliases,
            },
            scope_rules.as_ref(),
        );
        (candidates, guardrails.forced_type.clone())
    } else {
        (Vec::new(), None)
    };

    // construct prompt
    let prompt = if use_classifier {
        construct_classifier_prompt(
            diff_info,
            &intelligence,
            &scope_candidates,
            forced_type.as_deref(),
            effective_scope_strategy,
            ai_control,
            scope_aliases,
            &guardrails,
            scope_rules.as_ref(),
        )
    } else {
        construct_intelligent_prompt(diff_info, &intelligence)
    };

    if is_debug {
        print_debug_info(diff_info, &intelligence, &prompt);
    }

    let api_key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;

    let max_retries = 3;
    let mut retry_count = 0;
    // capture classification metadata for audit fields (only set when use_classifier=true)
    let mut captured_scope_reason: Option<String> = None;
    let mut captured_scope_confidence: Option<f32> = None;
    let mut captured_overall_confidence: Option<f32> = None;

    let result = loop {
        let current_prompt = if retry_count > 0 {
            if use_classifier {
                format!(
                    "{prompt}\n\nimportant: output must be valid json with the exact schema. no markdown."
                )
            } else {
                let scope_line = if let Some(scope) = &intelligence.scope_hint {
                    format!("must use scope: {scope}")
                } else {
                    "do not include a scope".to_string()
                };
                format!(
                    "{}\n\nimportant: the description must be under 72 characters. be concise.\nmust use type: {}\n{}",
                    prompt, intelligence.commit_type_hint, scope_line
                )
            }
        } else {
            prompt.clone()
        };

        let system_prompt = if use_classifier {
            "you are a precise classifier. output json only, no markdown or commentary."
                .to_string()
        } else {
            get_system_prompt(&intelligence).to_string()
        };

        let request = OpenRouterRequest {
            model: model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: system_prompt,
                },
                Message {
                    role: "user".to_string(),
                    content: current_prompt,
                },
            ],
            temperature: Some(0.1),
            top_p: Some(0.9),
            max_tokens: Some(400),
            stop: None,
        };

        let response = match make_api_request(&api_key, request).await {
            Ok(resp) => resp,
            Err(e) => break Err(e),
        };

        let raw_response = match response.choices.first() {
            Some(choice) => choice.message.content.clone(),
            None => {
                eprintln!("Warning: Empty choices in response");
                String::new()
            }
        };

        if is_debug {
            println!("🐛 debug: raw api response:");
            println!("═══════════════════════════════════════");
            println!("{raw_response}");
            println!("═══════════════════════════════════════");
            println!();
        }

        let mut commit_msg = if use_classifier {
            match parse_commit_classification(&raw_response) {
                Ok(mut classification) => {
                    if let Some(min_confidence) = min_confidence {
                        if classification.confidence < min_confidence {
                            let error = anyhow::anyhow!(
                                "classifier confidence too low ({:.2} < {:.2})",
                                classification.confidence,
                                min_confidence
                            );
                            if retry_count < max_retries {
                                retry_count += 1;
                                if is_debug {
                                    println!(
                                        "⚠️  {error}, retrying ({retry_count}/{max_retries})\n"
                                    );
                                }
                                continue;
                            }
                            return Err(error);
                        }
                    }
                    if let Some(min_scope_confidence) = min_scope_confidence {
                        if classification.scope.is_some()
                            && classification.scope_confidence < min_scope_confidence
                        {
                            classification.scope = None;
                            classification.scope_reason = None;
                        }
                    }
                    if let Some(forced_type) = forced_type.as_ref() {
                        classification.commit_type = forced_type.to_string();
                    }
                    if let Some(scope) = classification.scope.as_ref() {
                        let preferred_scope = prefer_short_scope(scope, &scope_candidates);
                        if scope_candidates.contains(&preferred_scope) {
                            classification.scope = Some(preferred_scope);
                        } else if effective_scope_strategy == ScopeStrategy::Conservative {
                            classification.scope = None;
                            classification.scope_reason = None;
                        } else if ai_control != AiControl::High && is_debug {
                            println!(
                                "⚠️  classifier scope '{}' not in suggested candidates",
                                scope
                            );
                        }
                    }
                    if let Some(scope_rules) = scope_rules.as_ref() {
                        if scope_rules.omit_scope_if_single()
                            && scope_candidates.len() <= 1
                            && classification.scope.is_some()
                        {
                            classification.scope = None;
                            classification.scope_reason = None;
                        }

                        if let Some(scope) = classification.scope.as_ref() {
                            if scope_rules.omit_scope_if_ambiguous() {
                                let coverage = calculate_scope_coverage_with_rules(
                                    diff_info,
                                    scope,
                                    Some(scope_rules),
                                );
                                if coverage < 0.7 {
                                    classification.scope = None;
                                    classification.scope_reason = None;
                                }
                            }
                        }
                    }
                    // capture audit fields before rendering
                    captured_scope_reason = if classification.scope.is_some() {
                        classification.scope_reason.clone()
                    } else {
                        None
                    };
                    captured_scope_confidence = if classification.scope.is_some() {
                        Some(classification.scope_confidence)
                    } else {
                        None
                    };
                    captured_overall_confidence = Some(classification.confidence);

                    match render_commit_message_from_classification(&classification) {
                        Ok(message) => message,
                        Err(e) => {
                            if retry_count < max_retries {
                                retry_count += 1;
                                if is_debug {
                                    println!(
                                        "⚠️  classifier render failed: {e}, retrying ({retry_count}/{max_retries})\n"
                                    );
                                }
                                continue;
                            }
                            return Err(e.context("failed to render classifier response"));
                        }
                    }
                }
                Err(e) => {
                    if retry_count < max_retries {
                        retry_count += 1;
                        if is_debug {
                            println!(
                                "⚠️  classifier parsing failed: {e}, retrying ({retry_count}/{max_retries})\n"
                            );
                        }
                        continue;
                    }
                    return Err(e.context("failed to parse classifier response"));
                }
            }
        } else {
            let commit_msg = extract_commit_message(&raw_response);
            post_process_commit_message(&commit_msg)
        };

        if use_classifier {
            commit_msg = post_process_commit_message(&commit_msg);
        }

        if is_debug {
            println!("🐛 debug: extracted and processed commit message:");
            println!("═══════════════════════════════════════");
            println!("'{commit_msg}");
            println!("═══════════════════════════════════════");
            println!();
        }

        // check if type matches hint (legacy path only)
        let (generated_type, expected_type) = if use_classifier {
            (String::new(), String::new())
        } else {
            let generated_type = commit_msg
                .split(':')
                .next()
                .unwrap_or("")
                .split('(')
                .next()
                .unwrap_or("")
                .split('!')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let expected_type = intelligence.commit_type_hint.clone();
            (generated_type, expected_type)
        };

        // first try to validate as-is
        match validate_commit_message(&commit_msg) {
            Ok(()) => {
                if use_classifier {
                    break Ok(commit_msg);
                }
                if generated_type == expected_type {
                    break Ok(commit_msg);
                } else if retry_count < max_retries {
                    retry_count += 1;
                    if is_debug {
                        println!("⚠️  generated type '{generated_type}' doesn't match expected '{expected_type}', retrying ({retry_count}/{max_retries})\n");
                    }
                    continue;
                } else {
                    break Ok(commit_msg); // accept even if type doesn't match after retries
                }
            }
            Err(e) => {
                // try to auto-fix common formatting issues
                if let Ok(fixed_msg) = fix_commit_format(&commit_msg) {
                    if is_debug {
                        println!("🔧 auto-fixed commit format:");
                        println!("  original: {commit_msg}");
                        println!("  fixed: {fixed_msg}");
                    }
                    // validate the fixed message
                    if validate_commit_message(&fixed_msg).is_ok() {
                        break Ok(fixed_msg);
                    }
                }

                // if we couldn't fix it, handle specific errors
                if e.to_string().contains("description too long") && retry_count < max_retries {
                    retry_count += 1;
                    if is_debug {
                        println!(
                            "⚠️  description too long, retrying ({retry_count}/{max_retries})\n"
                        );
                    }
                    continue;
                } else if e.to_string().contains("invalid scope") && retry_count < max_retries {
                    // try again but force a concrete scope if we have none
                    retry_count += 1;
                    if is_debug {
                        println!(
                            "⚠️  invalid scope, retrying with stricter guidance ({retry_count}/{max_retries})\n"
                        );
                    }
                    continue;
                } else {
                    break Err(e);
                }
            }
        }
    };

    if let Some(spinner) = spinner {
        spinner.finish_and_clear();
    }
    result.map(|message| GenerationResult {
        message,
        scope_reason: captured_scope_reason,
        scope_confidence: captured_scope_confidence,
        overall_confidence: captured_overall_confidence,
    })
}

/// make api request to openrouter
async fn make_api_request(api_key: &str, request: OpenRouterRequest) -> Result<OpenRouterResponse> {
    let max_retries = 3;
    let mut retry_delay = Duration::from_secs(1);

    for attempt in 0..max_retries {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = resp
                        .json::<OpenRouterResponse>()
                        .await
                        .context("failed to parse openrouter api response")?;
                    return Ok(body);
                } else {
                    let status = resp.status();
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "unknown error".to_string());

                    if status.is_server_error() || status == 429 {
                        if attempt < max_retries - 1 {
                            eprintln!(
                                "Retryable error ({status}): {error_text}. Retrying in {retry_delay:?}..."
                            );
                            sleep(retry_delay).await;
                            retry_delay *= 2;
                            continue;
                        }
                    } else if status == 400 && error_text.to_lowercase().contains("model") {
                        return Err(anyhow::anyhow!(
                            "invalid model '{}'. use the model settings menu to select a different model",
                            request.model
                        ));
                    }

                    return Err(anyhow::anyhow!(
                        "openrouter api error ({}): {}",
                        status,
                        error_text
                    ));
                }
            }
            Err(e) => {
                if attempt < max_retries - 1 {
                    eprintln!("Network error: {e}. Retrying in {retry_delay:?}...");
                    sleep(retry_delay).await;
                    retry_delay *= 2;
                    continue;
                } else {
                    return Err(anyhow::anyhow!(
                        "failed to connect to openrouter api after {max_retries} attempts: {e}"
                    ));
                }
            }
        }
    }

    Err(anyhow::anyhow!(
        "failed to complete api request after {max_retries} attempts"
    ))
}

fn prefer_short_scope(scope: &str, scope_candidates: &[String]) -> String {
    if let Some(idx) = scope.rfind('-') {
        let suffix = &scope[idx + 1..];
        if !suffix.is_empty() && scope_candidates.contains(&suffix.to_string()) {
            return suffix.to_string();
        }
    }
    scope.to_string()
}

/// print debug information
fn print_debug_info(diff_info: &DiffInfo, intelligence: &CommitIntelligence, prompt: &str) {
    println!("🐛 debug: commit intelligence analysis:");
    println!(
        "  └─ complexity score: {:.1}/5.0",
        intelligence.complexity_score
    );
    println!("  └─ requires body: {}", intelligence.requires_body);
    println!(
        "  └─ detected patterns: {}",
        intelligence.detected_patterns.len()
    );
    for pattern in &intelligence.detected_patterns {
        println!(
            "     • {}: {} (impact: {:.1})",
            format_pattern_type(&pattern.pattern_type),
            pattern.description,
            pattern.impact
        );
    }
    println!("  └─ suggested type: {}", intelligence.commit_type_hint);
    if let Some(scope) = &intelligence.scope_hint {
        println!("  └─ suggested scope: {scope}");
    }
    println!();

    println!("🐛 debug: file analysis summary:");
    for (i, file) in diff_info.files.iter().enumerate() {
        if i >= 3 {
            println!("  ... and {} more files", diff_info.files.len() - i);
            break;
        }
        println!(
            "  └─ {}: +{} -{} lines",
            file.path, file.added_lines, file.removed_lines
        );

        // show what diff content is being sent to AI
        let lines_to_include = if file.added_lines + file.removed_lines > 100 {
            15
        } else {
            20
        };
        let meaningful_diff = extract_meaningful_diff_lines(&file.diff_content, lines_to_include);
        if !meaningful_diff.is_empty() {
            println!(
                "     📝 diff sent to ai ({} lines):",
                meaningful_diff.lines().count()
            );
            for (j, line) in meaningful_diff.lines().enumerate() {
                match j.cmp(&3) {
                    std::cmp::Ordering::Less => {
                        // show first 3 lines as sample
                        println!("       {line}");
                    }
                    std::cmp::Ordering::Equal => {
                        println!(
                            "       ... ({} more lines sent to ai)",
                            meaningful_diff.lines().count() - 3
                        );
                        break;
                    }
                    std::cmp::Ordering::Greater => break,
                }
            }
        }
    }
    println!();

    println!("🐛 debug: full prompt being sent to ai:");
    println!("═══════════════════════════════════════");
    println!("{prompt}");
    println!("═══════════════════════════════════════");
    println!();
}

fn format_pattern_type(pattern_type: &PatternType) -> &'static str {
    match pattern_type {
        PatternType::NewFilePattern => "new file",
        PatternType::MassModification => "mass modification",
        PatternType::CrossLayerChange => "cross-layer",
        PatternType::InterfaceEvolution => "interface evolution",
        PatternType::ArchitecturalShift => "architectural",
        PatternType::ConfigurationDrift => "configuration",
        PatternType::DependencyUpdate => "dependency",
        PatternType::RefactoringPattern => "refactoring",
        PatternType::FeatureAddition => "feature",
        PatternType::BugFixPattern => "bugfix",
        PatternType::TestEvolution => "test",
        PatternType::DocumentationUpdate => "documentation",
        PatternType::StyleNormalization => "style",
        PatternType::PerformanceTuning => "performance",
        PatternType::SecurityHardening => "security",
        PatternType::CiChange => "ci/cd",
        PatternType::Deprecation => "deprecation",
        PatternType::SecurityFix => "security fix",
    }
}
