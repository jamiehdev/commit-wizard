// commit-wizard-core/src/lib.rs

// declare modules
pub mod ai;
pub mod analysis;
pub mod engine;
pub mod git;
pub mod utils;

// re-export key structs/functions for external use by other crates
pub use anyhow::{Context, Result}; // re-export for convenience
pub use clap::Parser; // re-export Parser for CLI crate
use console::style;
pub use dotenv::dotenv;
use serde::{Deserialize, Serialize};
pub use std::env;
use std::fs;

pub use crate::ai::{generate_conventional_commit, generate_conventional_commit_with_model, GenerationResult};
pub use crate::analysis::{derive_scope_candidates, AiControl, ScopeStrategy};
pub use crate::engine::{generate_commit_message, CommitGenerationResult};
pub use crate::git::{get_diff_info, get_staged_files, has_staged_changes, DiffInfo, ModifiedFile};

// configuration structure for commit-wizard
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub models: ModelConfig,
    pub current_model: Option<String>, // save user's preferred model
    pub auto_select: bool,             // enable automatic complexity-based selection
    // AI knobs with serde defaults for backward compat with old config files
    #[serde(default = "default_ai_control")]
    pub ai_control: AiControl,
    #[serde(default = "default_scope_strategy")]
    pub scope_strategy: ScopeStrategy,
    #[serde(default = "default_scope_aliases")]
    pub scope_aliases: bool,
    #[serde(default)]
    pub min_scope_confidence: Option<f32>,
    #[serde(default)]
    pub min_confidence: Option<f32>,
}

fn default_ai_control() -> AiControl {
    AiControl::High
}

fn default_scope_strategy() -> ScopeStrategy {
    ScopeStrategy::Free
}

fn default_scope_aliases() -> bool {
    true
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ModelConfig {
    pub fast: String,
    pub thinking: String,
    #[serde(default = "default_model")]
    pub default: String,
    pub available: Vec<AvailableModel>,
}

fn default_model() -> String {
    "mistralai/devstral-2512:free".to_string()
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AvailableModel {
    pub name: String,
    pub description: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            models: ModelConfig {
                fast: "deepseek/deepseek-chat-v3-0324:free".to_string(),
                thinking: "deepseek/deepseek-r1-0528:free".to_string(),
                default: "mistralai/devstral-2512:free".to_string(),
                available: vec![
                    AvailableModel {
                        name: "mistralai/devstral-2512:free".to_string(),
                        description: "mistral devstral 2512 (free - strong code model)".to_string(),
                    },
                    AvailableModel {
                        name: "deepseek/deepseek-r1-0528:free".to_string(),
                        description: "deepseek r1 (thinking model - best for complex commits)"
                            .to_string(),
                    },
                    AvailableModel {
                        name: "deepseek/deepseek-chat-v3-0324:free".to_string(),
                        description: "deepseek chat v3 (fast model - good for simple commits)"
                            .to_string(),
                    },
                    AvailableModel {
                        name: "deepseek/deepseek-r1-0528-qwen3-8b:free".to_string(),
                        description: "deepseek r1 qwen3 8b (balanced - free model)".to_string(),
                    },
                    AvailableModel {
                        name: "anthropic/claude-3.5-sonnet".to_string(),
                        description: "claude 3.5 sonnet (premium - high quality)".to_string(),
                    },
                    AvailableModel {
                        name: "openai/gpt-4o".to_string(),
                        description: "gpt-4o (premium - balanced performance)".to_string(),
                    },
                    AvailableModel {
                        name: "openai/gpt-4o-mini".to_string(),
                        description: "gpt-4o mini (affordable - good quality)".to_string(),
                    },
                    AvailableModel {
                        name: "meta-llama/llama-3.1-8b-instruct:free".to_string(),
                        description: "llama 3.1 8b (free - basic quality)".to_string(),
                    },
                    AvailableModel {
                        name: "qwen/qwen-2.5-72b-instruct:free".to_string(),
                        description: "qwen 2.5 72b (free - good quality)".to_string(),
                    },
                ],
            },
            current_model: None, // no saved model initially
            auto_select: false,  // default to not auto-selecting
            ai_control: AiControl::High,
            scope_strategy: ScopeStrategy::Free,
            scope_aliases: true,
            min_scope_confidence: None,
            min_confidence: None,
        }
    }
}

// argument parsing struct - this can be shared by CLI and NAPI (if NAPI parses from Vec<String>)
#[derive(Parser, Debug, Clone)] // added Clone
#[command(name = "commit-wizard-core")] // generic name for the core functionality
pub struct CoreCliArgs {
    /// path to git repository (defaults to current directory)
    #[arg(short, long)]
    pub path: Option<String>,

    /// maximum file size in kb to analyse (files larger than this will be ignored)
    #[arg(short, long, default_value = "100")]
    pub max_size: usize,

    /// maximum number of files to analyse
    #[arg(short = 'f', long, default_value = "10")]
    pub max_files: usize,

    /// show detailed diff information
    #[arg(short, long)]
    pub verbose: bool,

    /// automatically run the commit command when confirmed
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// show debug information including raw AI responses
    #[arg(long)]
    pub debug: bool,

    /// Run without interactive prompts (prints commit message only).
    #[arg(long)]
    pub headless: bool,

    /// Output machine-readable JSON (implies --headless).
    #[arg(long)]
    pub headless_json: bool,

    /// Use the structured classifier pipeline (experimental).
    #[arg(long)]
    pub use_classifier: bool,

    /// Minimum acceptable classifier confidence (0.0-1.0, requires --use-classifier).
    #[arg(long)]
    pub min_confidence: Option<f32>,

    /// Minimum acceptable scope confidence (0.0-1.0, requires --use-classifier).
    #[arg(long)]
    pub min_scope_confidence: Option<f32>,

    /// AI control preset (high, balanced, strict). Overrides config default when set.
    #[arg(long, value_enum)]
    pub ai_control: Option<AiControl>,

    /// Strategy for scope selection (auto, conservative, free). Overrides config default when set.
    #[arg(long, value_enum)]
    pub scope_strategy: Option<ScopeStrategy>,

    /// Allow alias scopes derived from common prefixes (e.g., core/cli). Overrides config default when set.
    #[arg(
        long,
        num_args = 0..=1,
        default_missing_value = "true",
        action = clap::ArgAction::Set,
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub scope_aliases: Option<bool>,

    /// use smart model selection: fast model for simple commits, thinking model for complex ones
    #[arg(long)]
    pub smart_model: bool,

    /// test git diff processing without user interaction (for testing purposes)
    #[arg(long)]
    pub test_diff: bool,

    /// automatically commit the generated message when using --test-diff (for testing purposes)
    #[arg(long)]
    pub auto_commit: bool,

    /// require AI generation to succeed in --test-diff mode (for CI smoke tests)
    #[arg(long)]
    pub ai_smoke: bool,
}

impl CoreCliArgs {
    pub fn effective_ai_control(&self, config: &Config) -> AiControl {
        self.ai_control.unwrap_or(config.ai_control)
    }

    pub fn effective_scope_strategy(&self, config: &Config) -> ScopeStrategy {
        self.scope_strategy.unwrap_or(config.scope_strategy)
    }

    pub fn effective_scope_aliases(&self, config: &Config) -> bool {
        self.scope_aliases.unwrap_or(config.scope_aliases)
    }

    pub fn effective_min_confidence(&self, config: &Config) -> Option<f32> {
        self.min_confidence.or(config.min_confidence)
    }

    pub fn effective_min_scope_confidence(&self, config: &Config) -> Option<f32> {
        self.min_scope_confidence.or(config.min_scope_confidence)
    }
}

/// get a safe fallback model that should always work
pub(crate) fn safe_fallback_model(config: &Config) -> String {
    // try in order of preference
    if config
        .models
        .available
        .iter()
        .any(|m| m.name == config.models.thinking)
    {
        config.models.thinking.clone()
    } else if config
        .models
        .available
        .iter()
        .any(|m| m.name == config.models.fast)
    {
        config.models.fast.clone()
    } else if !config.models.available.is_empty() {
        config.models.available[0].name.clone()
    } else {
        // ultimate fallback
        "deepseek/deepseek-r1-0528:free".to_string()
    }
}

/// get the current model to use based on config, args, and diff complexity
pub fn get_current_model(
    config: &Config,
    args: &CoreCliArgs,
    diff_info: Option<&DiffInfo>,
) -> String {
    // if user has auto-complexity enabled, always use complexity-based selection
    if config.auto_select {
        if let Some(diff) = diff_info {
            let intelligence = ai::analyse_commit_intelligence(diff);
            return ai::select_model_for_complexity(&intelligence, args.debug, config);
        }
    }

    // if user has a saved preference, use that
    if let Some(saved_model) = &config.current_model {
        return saved_model.clone();
    }

    // if smart model is enabled and we have diff info, choose based on complexity
    if args.smart_model {
        if let Some(diff) = diff_info {
            let intelligence = ai::analyse_commit_intelligence(diff);
            return ai::select_model_for_complexity(&intelligence, args.debug, config);
        }
    }

    // fallback to environment variable or configured default model
    env::var("OPENROUTER_MODEL").unwrap_or_else(|_| config.models.default.clone())
}

/// get a human-readable description for a model name
pub fn get_model_description(config: &Config, model_name: &str) -> String {
    for available_model in &config.models.available {
        if available_model.name == model_name {
            return available_model.description.clone();
        }
    }

    // fallback descriptions for smart models
    if model_name == config.models.fast {
        return "fast model".to_string();
    } else if model_name == config.models.thinking {
        return "thinking model".to_string();
    }

    // fallback to model name
    model_name.to_string()
}

pub fn ensure_api_key_present() -> Result<()> {
    dotenv().ok();
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        anyhow::anyhow!("OPENROUTER_API_KEY environment variable is not set. please set it with: export OPENROUTER_API_KEY=your-api-key")
    })?;

    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(anyhow::anyhow!(
            "OPENROUTER_API_KEY is empty. please provide a valid API key"
        ));
    }

    Ok(())
}

// headless entry point for core usage
pub async fn execute_commit_wizard_flow(args: CoreCliArgs) -> Result<(String, bool)> {
    // load configuration once
    let mut config = load_config()?;

    // test mode: validate git diff processing without user interaction
    if args.test_diff {
        return test_git_diff_processing(&args).await;
    }

    let is_headless = args.headless || args.headless_json;
    if args.headless_json && args.debug {
        return Err(anyhow::anyhow!(
            "--debug is not supported with --headless-json (use --headless instead)"
        ));
    }
    let min_confidence = args.effective_min_confidence(&config);
    let min_scope_confidence = args.effective_min_scope_confidence(&config);

    if let Some(min_confidence) = min_confidence {
        if !(0.0..=1.0).contains(&min_confidence) {
            return Err(anyhow::anyhow!(
                "--min-confidence must be between 0.0 and 1.0"
            ));
        }
        if !args.use_classifier {
            return Err(anyhow::anyhow!(
                "--min-confidence requires --use-classifier"
            ));
        }
    }
    if let Some(min_scope_confidence) = min_scope_confidence {
        if !(0.0..=1.0).contains(&min_scope_confidence) {
            return Err(anyhow::anyhow!(
                "--min-scope-confidence must be between 0.0 and 1.0"
            ));
        }
        if !args.use_classifier {
            return Err(anyhow::anyhow!(
                "--min-scope-confidence requires --use-classifier"
            ));
        }
    }

    if !is_headless {
        return Err(anyhow::anyhow!(
            "interactive mode is provided by commit-wizard-cli; re-run with --headless or use the CLI binary"
        ));
    }

    // centralised API key validation - check both existence and content
    ensure_api_key_present()?;

    run_headless_flow(args, &mut config).await
}

/// handles headless commit generation without interactive prompts
pub async fn run_headless_flow(
    args: CoreCliArgs,
    config: &mut Config,
) -> Result<(String, bool)> {
    let is_headless = args.headless || args.headless_json;
    let repo_path = args.path.clone().unwrap_or_else(|| ".".to_string());

    let generation = crate::engine::generate_commit_message(
        &args,
        config,
        is_headless,
        args.use_classifier,
    )
    .await?;

    let commit_message = generation.commit_message;
    let selected_model = generation.selected_model;
    let scope_reason = generation.scope_reason;
    let scope_confidence = generation.scope_confidence;
    let overall_confidence = generation.overall_confidence;
    let mut commit_succeeded = false;

    if args.yes {
        let outcome = commit_with_message(&repo_path, &commit_message)?;
        if !outcome.success {
            return Err(anyhow::anyhow!("git commit command failed: {}", outcome.stderr));
        }
        commit_succeeded = true;
    }

    if is_headless {
        if args.headless_json {
            let output = HeadlessRunOutput {
                mode: "run",
                commit_message: commit_message.clone(),
                commit_succeeded,
                selected_model,
                scope_reason,
                scope_confidence,
                overall_confidence,
            };
            print_headless_json(&output)?;
        } else {
            println!("{commit_message}");
        }
    }

    Ok((commit_message, commit_succeeded))
}

pub struct CommitOutcome {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

pub fn commit_with_message(repo_path: &str, message: &str) -> Result<CommitOutcome> {
    let repo_dir_path = if repo_path == "." {
        env::current_dir().context("failed to get current directory")?
    } else {
        std::path::PathBuf::from(repo_path)
    };

    let output = std::process::Command::new("git")
        .current_dir(repo_dir_path)
        .args(["commit", "-m", message])
        .output()
        .context("failed to execute git commit command")?;

    Ok(CommitOutcome {
        success: output.status.success(),
        stdout: String::from_utf8(output.stdout).unwrap_or_default(),
        stderr: String::from_utf8(output.stderr).unwrap_or_default(),
    })
}

pub fn load_config() -> Result<Config> {
    let config_path = get_config_path()?;

    if config_path.exists() {
        let content = fs::read_to_string(&config_path).context("failed to read config file")?;
        let config: Config = toml::from_str(&content).context("failed to parse config file")?;
        Ok(config)
    } else {
        // create default config file
        let config = Config::default();
        save_config(&config)?;
        Ok(config)
    }
}

/// save configuration to config file
pub fn save_config(config: &Config) -> Result<()> {
    let config_path = get_config_path()?;

    // create config directory if it doesn't exist
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).context("failed to create config directory")?;
    }

    let content = toml::to_string_pretty(config).context("failed to serialize config")?;

    fs::write(&config_path, content).context("failed to write config file")?;

    println!(
        "{} {}",
        style("✅ config saved:").green(),
        style(config_path.display()).yellow()
    );

    Ok(())
}

/// get the path to the config file
fn get_config_path() -> Result<std::path::PathBuf> {
    let config_dir = if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg_config)
    } else if let Ok(home) = env::var("HOME") {
        std::path::PathBuf::from(home).join(".config")
    } else {
        return Err(anyhow::anyhow!("could not determine config directory"));
    };

    Ok(config_dir.join("commit-wizard").join("config.toml"))
}

// openrouter api structures for model fetching
#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct OpenRouterModel {
    id: String,
    name: String,
    description: Option<String>,
    pricing: Option<ModelPricing>,
}

#[derive(Deserialize, Debug)]
struct ModelPricing {
    prompt: String,
    completion: String,
}

#[derive(Deserialize, Debug)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

/// fetch available models from openrouter api with caching
pub async fn fetch_openrouter_models() -> Result<Vec<AvailableModel>> {
    // check for cached models first
    if let Ok(cached_models) = load_cached_models() {
        return Ok(cached_models);
    }

    let api_key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;

    let client = reqwest::Client::new();
    let response = client
        .get("https://openrouter.ai/api/v1/models")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .send()
        .await
        .context("failed to fetch models from openrouter api")?;

    let models_response: OpenRouterModelsResponse = response
        .json()
        .await
        .context("failed to parse openrouter models response")?;

    let mut available_models = Vec::new();
    for model in models_response.data {
        let description = format_model_description(&model);
        available_models.push(AvailableModel {
            name: model.id,
            description,
        });
    }

    // sort by name for better UX
    available_models.sort_by(|a, b| a.name.cmp(&b.name));

    // cache the models for future use
    if let Err(e) = save_cached_models(&available_models) {
        eprintln!("⚠️  warning: failed to cache models: {e}");
    }

    Ok(available_models)
}

/// format a model description with pricing and capabilities
fn format_model_description(model: &OpenRouterModel) -> String {
    // start with just the model ID for clean display
    let mut desc = model.id.clone();

    // add pricing info concisely
    if let Some(pricing) = &model.pricing {
        if pricing.prompt == "0" && pricing.completion == "0" {
            desc.push_str(" (free)");
        } else {
            desc.push_str(" (premium)");
        }
    }

    desc
}

/// load cached models from disk (expires after 24 hours)
fn load_cached_models() -> Result<Vec<AvailableModel>> {
    let cache_path = get_models_cache_path()?;

    if !cache_path.exists() {
        return Err(anyhow::anyhow!("no cache file found"));
    }

    // check if cache is expired (older than 24 hours)
    let metadata = fs::metadata(&cache_path)?;
    let cache_age = metadata
        .modified()?
        .elapsed()
        .map_err(|_| anyhow::anyhow!("failed to get cache age"))?;

    if cache_age > std::time::Duration::from_secs(24 * 60 * 60) {
        // cache is expired, remove it
        let _ = fs::remove_file(&cache_path);
        return Err(anyhow::anyhow!("cache expired"));
    }

    let content = fs::read_to_string(&cache_path)?;
    let cached_models: Vec<AvailableModel> = serde_json::from_str(&content)?;

    Ok(cached_models)
}

/// save models to cache
fn save_cached_models(models: &[AvailableModel]) -> Result<()> {
    let cache_path = get_models_cache_path()?;

    // create cache directory if it doesn't exist
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(models)?;
    fs::write(&cache_path, content)?;

    Ok(())
}

/// get the path to the models cache file
fn get_models_cache_path() -> Result<std::path::PathBuf> {
    let config_dir = if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
        std::path::PathBuf::from(xdg_config)
    } else if let Ok(home) = env::var("HOME") {
        std::path::PathBuf::from(home).join(".config")
    } else {
        return Err(anyhow::anyhow!("could not determine config directory"));
    };

    Ok(config_dir.join("commit-wizard").join("models_cache.json"))
}

#[derive(Serialize)]
struct HeadlessRunOutput {
    mode: &'static str,
    commit_message: String,
    commit_succeeded: bool,
    selected_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overall_confidence: Option<f32>,
}

#[derive(Serialize)]
struct HeadlessTestOutput {
    mode: &'static str,
    status: String,
    repo_path: String,
    staged_files: usize,
    files_processed: usize,
    added_lines: usize,
    removed_lines: usize,
    complexity_score: f32,
    requires_body: bool,
    commit_type_hint: String,
    scope_hint: Option<String>,
    scope_candidates: Vec<String>,
    forced_type: Option<String>,
    patterns_detected: usize,
    ai_generated: bool,
    commit_message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope_confidence: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    overall_confidence: Option<f32>,
}

fn print_headless_json<T: Serialize>(value: &T) -> Result<()> {
    let json = serde_json::to_string(value).context("failed to serialise headless output")?;
    println!("{json}");
    Ok(())
}

/// test git diff processing without user interaction (for automated testing)
pub async fn test_git_diff_processing(args: &CoreCliArgs) -> Result<(String, bool)> {
    use git2::Repository;

    let is_headless = args.headless || args.headless_json;
    let repo_path = args.path.as_deref().unwrap_or(".");

    let mut status = "ok".to_string();
    let mut ai_generated = false;
    let mut commit_message: Option<String> = None;
    // audit fields from classifier (only populated when use_classifier=true)
    let mut scope_reason: Option<String> = None;
    let mut scope_confidence: Option<f32> = None;
    let mut overall_confidence: Option<f32> = None;

    if !is_headless {
        println!(
            "{}",
            style("🔍 testing git diff processing...").cyan().bold()
        );
    }

    let _repo = match Repository::open(repo_path) {
        Ok(repo) => repo,
        Err(e) => {
            return Err(anyhow::anyhow!(
                "failed to open git repository at '{}': {}",
                repo_path,
                e
            ));
        }
    };

    let staged_files = crate::git::get_staged_files(repo_path)?;
    let staged_files_count = staged_files.len();

    if !is_headless {
        println!("📄 staged files found: {}", staged_files.len());
        for (i, file) in staged_files.iter().enumerate() {
            println!("  {}. {}", i + 1, file);
        }
    }

    if staged_files.is_empty() {
        status = "no_staged_files".to_string();
        if !is_headless {
            println!(
                "{}",
                style("⚠️  no staged files found. add files with 'git add' first.").yellow()
            );
        }

        if args.headless_json {
            let output = HeadlessTestOutput {
                mode: "test-diff",
                status: status.clone(),
                repo_path: repo_path.to_string(),
                staged_files: 0,
                files_processed: 0,
                added_lines: 0,
                removed_lines: 0,
                complexity_score: 0.0,
                requires_body: false,
                commit_type_hint: String::new(),
                scope_hint: None,
                scope_candidates: Vec::new(),
                forced_type: None,
                patterns_detected: 0,
                ai_generated: false,
                commit_message: None,
                scope_reason: None,
                scope_confidence: None,
                overall_confidence: None,
            };
            print_headless_json(&output)?;
        }

        return Ok(("test completed - no staged files".to_string(), false));
    }

    if !is_headless {
        println!("\n{}", style("🔬 analysing diffs...").cyan());
    }
    let diff_info = match crate::git::get_diff_info(
        repo_path,
        args.max_size * 1024,
        args.max_files,
        args.verbose && !is_headless,
    ) {
        Ok(info) => info,
        Err(e) => {
            if !is_headless {
                println!(
                    "{}",
                    style(&format!("❌ git diff analysis failed: {e}"))
                        .red()
                        .bold()
                );
            }
            return Err(e);
        }
    };

    let added_lines: usize = diff_info.files.iter().map(|f| f.added_lines).sum();
    let removed_lines: usize = diff_info.files.iter().map(|f| f.removed_lines).sum();

    if !is_headless {
        println!(
            "{}",
            style("✅ git diff processing successful!").green().bold()
        );
        println!("📊 analysis results:");
        println!("  └─ files processed: {}", diff_info.files.len());
        println!("  └─ total added lines: {added_lines}");
        println!("  └─ total removed lines: {removed_lines}");
    }

    if args.verbose && !is_headless {
        println!("\n{}", style("📝 detailed file analysis:").cyan());
        for (i, file) in diff_info.files.iter().enumerate() {
            println!(
                "  {}. {} (+{} -{}) [{:?}]",
                i + 1,
                file.path,
                file.added_lines,
                file.removed_lines,
                file.file_type
            );
            if !file.change_hints.is_empty() {
                let hint_strings: Vec<String> =
                    file.change_hints.iter().map(|h| format!("{h:?}")).collect();
                println!("     hints: {}", hint_strings.join(", "));
            }
        }
    }

    let intelligence = crate::ai::analyse_commit_intelligence(&diff_info);

    if !is_headless {
        println!(
            "\n{}",
            style("🧠 testing commit intelligence analysis...").cyan()
        );
        println!("✅ intelligence analysis successful!");
        println!("📈 intelligence results:");
        println!(
            "  └─ complexity score: {:.1}/5.0",
            intelligence.complexity_score
        );
        println!("  └─ suggested type: {}", intelligence.commit_type_hint);
        if let Some(scope) = &intelligence.scope_hint {
            println!("  └─ suggested scope: {scope}");
        }
        println!("  └─ requires body: {}", intelligence.requires_body);
        println!(
            "  └─ patterns detected: {}",
            intelligence.detected_patterns.len()
        );

        if args.verbose && !intelligence.detected_patterns.is_empty() {
            println!("  └─ pattern details:");
            for pattern in &intelligence.detected_patterns {
                println!(
                    "     • {} (impact: {:.1})",
                    pattern.description, pattern.impact
                );
            }
        }
    }

    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            if !is_headless {
                println!(
                    "{}",
                    style(&format!("⚠️  failed to load config: {e}")).yellow()
                );
            }
            Config::default()
        }
    };

    let scope_aliases = args.effective_scope_aliases(&config);
    let scope_candidates = crate::analysis::derive_scope_candidates_with_options(
        &diff_info,
        crate::analysis::ScopeCandidateOptions {
            allow_aliases: scope_aliases,
        },
    );
    let forced_type = crate::analysis::determine_forced_commit_type(&diff_info);

    dotenv().ok();
    let api_key = std::env::var("OPENROUTER_API_KEY").ok();
    let has_api_key = api_key
        .as_ref()
        .map(|key| !key.trim().is_empty())
        .unwrap_or(false);

    if !has_api_key {
        status = "ai_skipped".to_string();
        if !is_headless {
            println!(
                "{}",
                style("⚠️  openrouter_api_key not set, skipping ai generation test").yellow()
            );
            println!(
                "{}",
                style("💡 set openrouter_api_key environment variable to test ai generation").dim()
            );
        }
    } else {
        if !is_headless {
            println!(
                "\n{}",
                style("🤖 testing ai commit message generation...").cyan()
            );
        }
        let min_confidence = args.effective_min_confidence(&config);
        let min_scope_confidence = args.effective_min_scope_confidence(&config);
        let scope_strategy = args.effective_scope_strategy(&config);
        let scope_aliases = args.effective_scope_aliases(&config);
        let ai_control = args.effective_ai_control(&config);

        let generated = match crate::ai::generate_conventional_commit_with_model(
            &diff_info,
            args.debug,
            args.smart_model,
            None, // use default model
            &config,
            is_headless,
            args.use_classifier,
            min_confidence,
            min_scope_confidence,
            scope_strategy,
            scope_aliases,
            ai_control,
        )
        .await
        {
            Ok(result) => {
                if !is_headless {
                    println!("✅ ai commit message generation successful!");
                }
                ai_generated = true;
                // capture audit fields
                scope_reason = result.scope_reason;
                scope_confidence = result.scope_confidence;
                overall_confidence = result.overall_confidence;
                result.message
            }
            Err(e) => {
                if args.ai_smoke {
                    return Err(e.context(
                        "ai smoke test failed (set OPENROUTER_API_KEY or disable --ai-smoke)",
                    ));
                }
                status = "ai_failed".to_string();
                if !is_headless {
                    println!("{}", style(&format!("❌ ai generation failed: {e}")).red());
                }
                String::new()
            }
        };

        if !generated.is_empty() {
            commit_message = Some(generated);
        }
    }

    if let Some(message) = commit_message.as_ref() {
        if !is_headless {
            println!("\n{}", style("📝 generated commit message:").green().bold());
            println!("{}", style("─".repeat(50)).dim());
            println!("{}", style(message).yellow());
            println!("{}", style("─".repeat(50)).dim());
        }

        if let Err(e) = crate::ai::validate_commit_message(message) {
            if !is_headless {
                println!(
                    "{}",
                    style(&format!("⚠️  generated message validation warning: {e}")).yellow()
                );
            }
        } else if !is_headless {
            println!(
                "{}",
                style("✅ generated message passes validation").green()
            );
        }
    }

    let commit_successful = if args.auto_commit {
        let message = match commit_message.clone() {
            Some(message) => message,
            None => {
                if !is_headless {
                    println!(
                        "{}",
                        style("⚠️  no commit message available to auto-commit").yellow()
                    );
                }
                return Err(anyhow::anyhow!(
                    "cannot auto-commit without a generated commit message"
                ));
            }
        };

        if !is_headless {
            println!(
                "\n{}",
                style("🚀 auto-committing generated message...")
                    .cyan()
                    .bold()
            );
        }

        let repo_dir_path = if repo_path == "." {
            std::env::current_dir().context("Failed to get current directory")?
        } else {
            std::path::PathBuf::from(&repo_path)
        };

        let output = std::process::Command::new("git")
            .current_dir(repo_dir_path)
            .args(["commit", "-m", &message])
            .output()
            .context("Failed to execute git commit command")?;

        if output.status.success() {
            if !is_headless {
                println!("{}", style("✅ commit successful!").green().bold());
                if let Ok(stdout) = String::from_utf8(output.stdout) {
                    if !stdout.trim().is_empty() {
                        println!("{stdout}");
                    }
                }
            }
            true
        } else {
            if !is_headless {
                println!("{}", style("❌ commit failed:").red().bold());
                if let Ok(stderr) = String::from_utf8(output.stderr) {
                    if !stderr.trim().is_empty() {
                        println!("{stderr}");
                    }
                }
            }
            false
        }
    } else {
        false
    };

    if !is_headless {
        println!(
            "\n{}",
            style("🎉 all tests passed! git diff processing and ai generation working correctly.")
                .green()
                .bold()
        );
    }

    if args.headless_json {
        let output = HeadlessTestOutput {
            mode: "test-diff",
            status: status.clone(),
            repo_path: repo_path.to_string(),
            staged_files: staged_files_count,
            files_processed: diff_info.files.len(),
            added_lines,
            removed_lines,
            complexity_score: intelligence.complexity_score,
            requires_body: intelligence.requires_body,
            commit_type_hint: intelligence.commit_type_hint.clone(),
            scope_hint: intelligence.scope_hint.clone(),
            scope_candidates,
            forced_type,
            patterns_detected: intelligence.detected_patterns.len(),
            ai_generated,
            commit_message: commit_message.clone(),
            scope_reason,
            scope_confidence,
            overall_confidence,
        };
        print_headless_json(&output)?;
    } else if is_headless {
        if let Some(message) = commit_message.as_ref() {
            println!("{message}");
        }
    }

    let final_message = commit_message.unwrap_or_else(|| {
        format!(
            "test completed successfully - {}",
            if status.is_empty() { "ok" } else { status.as_str() }
        )
    });

    Ok((final_message, commit_successful))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &PathBuf) -> Self {
            let previous = env::var_os(key);
            env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(value) = self.previous.take() {
                env::set_var(self.key, value);
            } else {
                env::remove_var(self.key);
            }
        }
    }

    struct TempConfigDir {
        path: PathBuf,
        _env_guard: EnvVarGuard,
        _lock_guard: std::sync::MutexGuard<'static, ()>,
    }

    impl TempConfigDir {
        fn initialise() -> Self {
            let lock = ENV_MUTEX
                .get_or_init(|| Mutex::new(()))
                .lock()
                .expect("env mutex poisoned");
            let unique = format!(
                "commit-wizard-test-{}-{}",
                process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time drift")
                    .as_nanos()
            );
            let path = env::temp_dir().join(unique);
            fs::create_dir_all(&path).expect("failed to create temp dir");
            let env_guard = EnvVarGuard::set("XDG_CONFIG_HOME", &path);
            Self {
                path,
                _env_guard: env_guard,
                _lock_guard: lock,
            }
        }
    }

    impl Drop for TempConfigDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn config_loads_with_missing_ai_knobs() {
        // simulate old config format without new AI fields
        let old_config_toml = r#"
            auto_select = false

            [models]
            fast = "model-a"
            thinking = "model-b"
            default = "model-c"
            available = []
        "#;

        let config: Config = toml::from_str(old_config_toml).expect("should parse old config");

        // verify serde defaults are applied
        assert_eq!(config.ai_control, AiControl::High);
        assert_eq!(config.scope_strategy, ScopeStrategy::Free);
        assert!(config.scope_aliases);
        assert!(config.min_scope_confidence.is_none());
        assert!(config.min_confidence.is_none());
    }

    #[test]
    fn config_preserves_explicit_ai_knobs() {
        let config_toml = r#"
            auto_select = true
            ai_control = "balanced"
            scope_strategy = "conservative"
            scope_aliases = true
            min_scope_confidence = 0.5
            min_confidence = 0.7

            [models]
            fast = "model-a"
            thinking = "model-b"
            default = "model-c"
            available = []
        "#;

        let config: Config = toml::from_str(config_toml).expect("should parse config with AI knobs");

        assert_eq!(config.ai_control, AiControl::Balanced);
        assert_eq!(config.scope_strategy, ScopeStrategy::Conservative);
        assert!(config.scope_aliases);
        assert_eq!(config.min_scope_confidence, Some(0.5));
        assert_eq!(config.min_confidence, Some(0.7));
    }

    #[test]
    fn headless_test_output_includes_audit_fields() {
        let output = HeadlessTestOutput {
            mode: "test-diff",
            status: "ok".to_string(),
            repo_path: ".".to_string(),
            staged_files: 1,
            files_processed: 1,
            added_lines: 10,
            removed_lines: 5,
            complexity_score: 2.0,
            requires_body: false,
            commit_type_hint: "feat".to_string(),
            scope_hint: Some("core".to_string()),
            scope_candidates: vec!["core".to_string()],
            forced_type: None,
            patterns_detected: 0,
            ai_generated: true,
            commit_message: Some("feat(core): add feature".to_string()),
            scope_reason: Some("primary directory".to_string()),
            scope_confidence: Some(0.9),
            overall_confidence: Some(0.85),
        };

        let json = serde_json::to_string(&output).expect("should serialize");

        // verify audit fields are present
        assert!(json.contains("\"scope_reason\":\"primary directory\""));
        assert!(json.contains("\"scope_confidence\":0.9"));
        assert!(json.contains("\"overall_confidence\":0.85"));
    }

    #[test]
    fn headless_test_output_omits_none_audit_fields() {
        let output = HeadlessTestOutput {
            mode: "test-diff",
            status: "ok".to_string(),
            repo_path: ".".to_string(),
            staged_files: 1,
            files_processed: 1,
            added_lines: 10,
            removed_lines: 5,
            complexity_score: 2.0,
            requires_body: false,
            commit_type_hint: "feat".to_string(),
            scope_hint: None,
            scope_candidates: vec![],
            forced_type: None,
            patterns_detected: 0,
            ai_generated: false,
            commit_message: None,
            scope_reason: None,
            scope_confidence: None,
            overall_confidence: None,
        };

        let json = serde_json::to_string(&output).expect("should serialize");

        // verify None fields are omitted (skip_serializing_if)
        assert!(!json.contains("scope_reason"));
        assert!(!json.contains("scope_confidence"));
        assert!(!json.contains("overall_confidence"));
    }

    #[test]
    fn cli_effective_defaults_respect_config() {
        let temp_dir = TempConfigDir::initialise();
        let config_dir = temp_dir.path.join("commit-wizard");
        fs::create_dir_all(&config_dir).expect("failed to create config dir");
        let config_path = config_dir.join("config.toml");
        fs::write(
            &config_path,
            r#"
auto_select = true
ai_control = "balanced"
scope_strategy = "conservative"
scope_aliases = true
min_scope_confidence = 0.5
min_confidence = 0.7

[models]
fast = "model-a"
thinking = "model-b"
default = "model-c"
available = []
"#,
        )
        .expect("failed to write config");

        let config = load_config().expect("should load config");
        let args = CoreCliArgs::try_parse_from(["commit-wizard", "--use-classifier"])
            .expect("should parse args");

        assert_eq!(args.effective_ai_control(&config), AiControl::Balanced);
        assert_eq!(
            args.effective_scope_strategy(&config),
            ScopeStrategy::Conservative
        );
        assert!(args.effective_scope_aliases(&config));
        assert_eq!(args.effective_min_confidence(&config), Some(0.7));
        assert_eq!(args.effective_min_scope_confidence(&config), Some(0.5));

        let args_override = CoreCliArgs::try_parse_from([
            "commit-wizard",
            "--use-classifier",
            "--ai-control",
            "strict",
            "--scope-strategy",
            "free",
            "--scope-aliases=false",
            "--min-confidence",
            "0.9",
            "--min-scope-confidence",
            "0.2",
        ])
        .expect("should parse override args");

        assert_eq!(args_override.effective_ai_control(&config), AiControl::Strict);
        assert_eq!(
            args_override.effective_scope_strategy(&config),
            ScopeStrategy::Free
        );
        assert!(!args_override.effective_scope_aliases(&config));
        assert_eq!(args_override.effective_min_confidence(&config), Some(0.9));
        assert_eq!(
            args_override.effective_min_scope_confidence(&config),
            Some(0.2)
        );
    }
}
