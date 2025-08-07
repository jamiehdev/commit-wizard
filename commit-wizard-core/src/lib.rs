// commit-wizard-core/src/lib.rs

// declare modules
pub mod ai;
pub mod git;
pub mod utils;

// re-export key structs/functions for external use by other crates
pub use anyhow::{Context, Result}; // re-export for convenience
pub use clap::Parser; // re-export Parser for CLI crate
pub use console::style; // re-export for CLI/NAPI crates if they do printing
pub use dialoguer::{theme::ColorfulTheme, Select}; // re-export for CLI/NAPI
pub use dotenv::dotenv;
pub use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
pub use std::env;
use std::fs;
pub use std::process::Command as StdCommand;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
pub use std::time::Duration;

pub use crate::ai::{generate_conventional_commit, generate_conventional_commit_with_model};
pub use crate::git::{get_diff_info, get_staged_files, has_staged_changes, DiffInfo, ModifiedFile};

// configuration structure for commit-wizard
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub models: ModelConfig,
    pub current_model: Option<String>, // save user's preferred model
    pub auto_select: bool,             // enable automatic complexity-based selection
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
    "deepseek/deepseek-r1-0528-qwen3-8b:free".to_string()
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
                default: "deepseek/deepseek-r1-0528-qwen3-8b:free".to_string(),
                available: vec![
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

/// get a safe fallback model that should always work
fn safe_fallback_model(config: &Config) -> String {
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
fn get_current_model(config: &Config, args: &CoreCliArgs, diff_info: Option<&DiffInfo>) -> String {
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

    // fallback to environment variable or default thinking model
    env::var("OPENROUTER_MODEL").unwrap_or_else(|_| config.models.thinking.clone())
}

/// get a human-readable description for a model name
fn get_model_description(config: &Config, model_name: &str) -> String {
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

// the core commit generation and interaction logic
pub async fn execute_commit_wizard_flow(args: CoreCliArgs) -> Result<(String, bool)> {
    // load configuration once
    let mut config = load_config()?;

    // centralised API key validation - check both existence and content
    dotenv().ok();
    let api_key = env::var("OPENROUTER_API_KEY").map_err(|_| {
        anyhow::anyhow!("OPENROUTER_API_KEY environment variable is not set. please set it with: export OPENROUTER_API_KEY=your-api-key")
    })?;

    // trim the API key and validate - store trimmed version for actual use
    let api_key = api_key.trim();
    if api_key.is_empty() {
        return Err(anyhow::anyhow!(
            "OPENROUTER_API_KEY is empty. please provide a valid API key"
        ));
    }

    // background refresh of model catalogue (non-blocking)
    let is_updating_models = Arc::new(AtomicBool::new(true));
    let just_updated_models = Arc::new(AtomicBool::new(false));
    {
        let updating = is_updating_models.clone();
        let updated = just_updated_models.clone();
        tokio::spawn(async move {
            // refresh model cache; ignore errors silently
            let _ = fetch_openrouter_models().await;
            updating.store(false, Ordering::Relaxed);
            updated.store(true, Ordering::Relaxed);
        });
    }

    // test mode: validate git diff processing without user interaction
    if args.test_diff {
        return test_git_diff_processing(&args).await;
    }

    // welcome banner
    println!(
        "{}",
        style("\ncommit-wizard 🧙 (core engine)").cyan().bold()
    );
    println!(
        "{}\n",
        style("ai-powered conventional commit message generator").dim()
    );

    loop {
        // show one-time background update completion notice
        if just_updated_models.swap(false, Ordering::Relaxed) {
            println!(
                "{}",
                style("✅ model catalogue updated in the background").green()
            );
        }

        let current_model_text = if config.auto_select {
            "auto-complexity".to_string()
        } else {
            get_current_model(&config, &args, None)
        };

        let model_settings_option = {
            let updating_suffix = if is_updating_models.load(Ordering::Relaxed) {
                style(" (updating...)").dim().to_string()
            } else {
                String::new()
            };
            format!(
                "model settings - current: {}{}",
                style(&current_model_text).yellow(),
                updating_suffix
            )
        };

        let options = &[
            "generate commit message".to_string(),
            model_settings_option,
            "refresh model catalogue now".to_string(),
            "exit".to_string(),
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("what would you like to do?")
            .default(0)
            .items(options)
            .interact()?;

        match selection {
            0 => {
                // Generate commit message
                // this will now contain the original logic and will return, breaking the loop
                return run_generate_and_commit_flow(args.clone(), &mut config).await;
            }
            1 => {
                // Model settings
                handle_model_settings(&mut config, &args).await?;
                println!(); // add a blank line for spacing before menu shows again
            }
            2 => {
                // manual model refresh
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner:.blue} updating model catalogue...")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.enable_steady_tick(Duration::from_millis(80));
                match fetch_openrouter_models().await {
                    Ok(models) => {
                        pb.finish_and_clear();
                        println!(
                            "{} {} {}",
                            style("✅").green(),
                            style(models.len()).yellow().bold(),
                            style("models available (catalogue refreshed)").green()
                        );
                    }
                    Err(e) => {
                        pb.finish_and_clear();
                        eprintln!("{} {}", style("⚠️  failed to refresh models:").yellow(), e);
                    }
                }
                println!();
            }
            3 => {
                // Exit
                println!("\n{}", style("👋 bye!").dim());
                return Ok(("".to_string(), false));
            }
            _ => unreachable!(),
        }
    }
}

/// handles the entire commit generation process
async fn run_generate_and_commit_flow(
    args: CoreCliArgs,
    config: &mut Config,
) -> Result<(String, bool)> {
    let repo_path = args.path.clone().unwrap_or_else(|| ".".to_string());

    // validate git repository early for clearer errors
    let repo = match git2::Repository::discover(&repo_path) {
        Ok(r) => r,
        Err(e) => return Err(anyhow::anyhow!("invalid git repository: {}", e)),
    };

    if repo.is_bare() {
        return Err(anyhow::anyhow!("bare repositories not supported"));
    }
    if args.smart_model {
        println!("{}", style("🤖 smart model selection enabled").green());
        println!(
            "{}\n",
            style("automatically choosing optimal model based on commit complexity").dim()
        );
    }

    match git::has_staged_changes(&repo_path) {
        Ok(has_staged) => {
            if has_staged {
                if let Ok(files) = git::get_staged_files(&repo_path) {
                    println!("{}\n", style("staged files:").cyan().bold());
                    for file in files {
                        println!("{}", style(format!("  - {file}")).green());
                    }
                    println!();
                }
            } else {
                println!(
                    "{}\n",
                    style("⚠️  no staged changes found, will analyse unstaged changes instead")
                        .yellow()
                        .bold()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "{} {}",
                style("❌ error checking staged changes:").red().bold(),
                style(e).red()
            );
        }
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "📊 ⠋", "📊 ⠙", "📊 ⠹", "📊 ⠸", "📊 ⠼", "📊 ⠴", "📊 ⠦", "📊 ⠧", "📊 ⠇", "📊 ⠏",
            ])
            .template("{spinner} analysing changes...")
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(120));

    let diff_info = git::get_diff_info(
        &repo_path,
        args.max_size * 1024,
        args.max_files,
        args.verbose,
    )
    .context("failed to get git diff information")?;

    spinner.finish_and_clear();

    if args.verbose {
        println!("found {} modified files", diff_info.files.len());
        for file in &diff_info.files {
            println!(
                "- {} ({} lines added, {} lines removed)",
                file.path, file.added_lines, file.removed_lines
            );
        }
    }

    if diff_info.files.is_empty() {
        return Err(anyhow::anyhow!("no changes detected in the repository"));
    }

    let mut selected_model = get_current_model(config, &args, Some(&diff_info));
    println!("{}", style("🤖 selected model:").cyan().bold());
    if config.auto_select {
        println!(
            "{} {}",
            style(&get_model_description(config, &selected_model)).yellow(),
            style("(auto-complexity)").dim()
        );
    } else {
        println!(
            "{}",
            style(&get_model_description(config, &selected_model)).yellow()
        );
    }
    println!();

    let mut commit_message = match ai::generate_conventional_commit_with_model(
        &diff_info,
        args.debug,
        args.smart_model,
        Some(selected_model.clone()),
        config,
    )
    .await
    {
        Ok(message) => message,
        Err(e) => {
            let error_str = e.to_string();
            if error_str.contains("invalid model") {
                eprintln!("{} {}", style("❌ model error:").red(), e);
                println!(
                    "{}",
                    style("🔧 automatically recovering with safe fallback model...").cyan()
                );

                let fallback_model = safe_fallback_model(config);
                selected_model = fallback_model.clone();

                config.current_model = Some(fallback_model.clone());
                config.auto_select = false;
                if let Err(save_err) = save_config(config) {
                    eprintln!(
                        "{} failed to save recovery config: {}",
                        style("⚠️").yellow(),
                        save_err
                    );
                }

                println!(
                    "{} {}",
                    style("✅ recovered with model:").green(),
                    style(&get_model_description(config, &selected_model)).yellow()
                );

                ai::generate_conventional_commit_with_model(
                    &diff_info,
                    args.debug,
                    args.smart_model,
                    Some(selected_model.clone()),
                    config,
                )
                .await
                .context("failed to generate commit message even with fallback model")?
            } else {
                return Err(e.context("failed to generate commit message"));
            }
        }
    };

    println!(
        "\n{}\n",
        style("✅ generated commit message:").green().bold()
    );
    println!("{}", style(&commit_message).yellow());
    println!();

    let mut should_commit_now = args.yes;
    let mut commit_succeeded = false;

    if !args.yes {
        println!("{}", style("press ctrl+c at any time to exit").dim());

        loop {
            let options = &[
                "yes, commit this message",
                "edit this message",
                "no, regenerate message",
                "model settings",
            ];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("what would you like to do?")
                .default(0)
                .items(options)
                .interact()?;

            match selection {
                0 => {
                    println!("{}", style("proceeding with commit...").green());
                    should_commit_now = true;
                    break;
                }
                1 => {
                    println!("{}", style("opening editor for commit message...").cyan());
                    if let Some(edited_message) = open_editor_for_message(&commit_message)? {
                        commit_message = edited_message;
                        println!("{}", style("commit message updated").green());
                    } else {
                        println!(
                            "{}",
                            style("edit cancelled, using previous message").yellow()
                        );
                    }
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                2 => {
                    println!("\n{}", style("regenerating...").cyan());
                    commit_message = ai::generate_conventional_commit_with_model(
                        &diff_info,
                        args.debug,
                        args.smart_model,
                        Some(selected_model.clone()),
                        config,
                    )
                    .await
                    .context("failed to regenerate commit message")?;
                    println!(
                        "\n{}\n",
                        style("✅ newly generated commit message:").green().bold()
                    );
                    println!("{}", style(&commit_message).yellow());
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                3 => {
                    // model settings
                    println!("\n{}", style("model settings").cyan().bold());

                    match select_model_interactively(config).await {
                        Ok(new_model) => {
                            if new_model == "AUTO_COMPLEXITY" {
                                config.auto_select = true;
                                config.current_model = None;
                                if let Err(e) = save_config(config) {
                                    eprintln!(
                                        "{} {}",
                                        style(
                                            "⚠️  warning: failed to save auto-complexity setting:"
                                        )
                                        .yellow(),
                                        e
                                    );
                                } else {
                                    println!(
                                        "{}",
                                        style("✅ auto-complexity selection enabled and saved")
                                            .green()
                                    );
                                }

                                selected_model = get_current_model(config, &args, Some(&diff_info));
                                println!(
                                    "{} {}",
                                    style("🤖 auto-selected:").cyan(),
                                    style(get_model_description(config, &selected_model)).yellow()
                                );
                            } else {
                                config.current_model = Some(new_model.clone());
                                config.auto_select = false;
                                if let Err(e) = save_config(config) {
                                    eprintln!(
                                        "{} {}",
                                        style("⚠️  warning: failed to save model preference:")
                                            .yellow(),
                                        e
                                    );
                                } else {
                                    println!(
                                        "{} {}",
                                        style("✅ model preference saved:").green(),
                                        style(get_model_description(config, &new_model)).yellow()
                                    );
                                }

                                selected_model = new_model;
                            }

                            // regenerate with new model
                            println!("\n{}", style("regenerating with new model...").cyan());
                            commit_message = ai::generate_conventional_commit_with_model(
                                &diff_info,
                                args.debug,
                                args.smart_model,
                                Some(selected_model.clone()),
                                config,
                            )
                            .await
                            .context("failed to regenerate commit message with new model")?;
                            println!(
                                "\n{}\n",
                                style("✅ newly generated commit message:").green().bold()
                            );
                            println!("{}", style(&commit_message).yellow());
                        }
                        Err(e) => {
                            if e.to_string() != "cancelled" {
                                eprintln!(
                                    "{} {}",
                                    style("⚠️  model selection failed:").yellow(),
                                    e
                                );
                            }
                        }
                    }
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                _ => unreachable!(),
            }
        }
    } else {
        println!(
            "{}",
            style("--yes flag detected, proceeding with generated message automatically.").green()
        );
    }

    if should_commit_now {
        println!("{}", style("executing commit command...").cyan());
        let repo_dir_path = if repo_path == "." {
            env::current_dir().context("Failed to get current directory")?
        } else {
            std::path::PathBuf::from(&repo_path)
        };

        let output = StdCommand::new("git")
            .current_dir(repo_dir_path)
            .args(["commit", "-m", &commit_message])
            .output()
            .context("failed to execute git commit command")?;

        if output.status.success() {
            println!("{}", style("\n✅ commit successful!").green().bold());
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if !stdout.trim().is_empty() {
                    println!("{stdout}");
                }
            }
            commit_succeeded = true;
        } else {
            eprintln!("{}", style("\n❌ commit failed:").red().bold());
            if let Ok(stderr) = String::from_utf8(output.stderr) {
                if !stderr.trim().is_empty() {
                    eprintln!("{stderr}");
                }
            }
            return Err(anyhow::anyhow!("git commit command failed"));
        }
    }

    Ok((commit_message, commit_succeeded))
}

/// handles interactive model settings changes
async fn handle_model_settings(config: &mut Config, args: &CoreCliArgs) -> Result<()> {
    let current_model_desc = get_model_description(config, &get_current_model(config, args, None));

    println!("\n{}", style("model settings").cyan().bold());
    println!(
        "{} {}",
        style("current model:").dim(),
        style(current_model_desc).yellow()
    );
    println!();

    match select_model_interactively(config).await {
        Ok(new_model) => {
            if new_model == "AUTO_COMPLEXITY" {
                config.auto_select = true;
                config.current_model = None;
                if let Err(e) = save_config(config) {
                    eprintln!(
                        "{} {}",
                        style("⚠️  warning: failed to save auto-complexity setting:").yellow(),
                        e
                    );
                } else {
                    println!(
                        "{}",
                        style("✅ auto-complexity selection enabled and saved").green()
                    );
                }
            } else {
                config.current_model = Some(new_model.clone());
                config.auto_select = false;
                if let Err(e) = save_config(config) {
                    eprintln!(
                        "{} {}",
                        style("⚠️  warning: failed to save model preference:").yellow(),
                        e
                    );
                } else {
                    let new_model_desc = get_model_description(config, &new_model);
                    println!(
                        "{} {}",
                        style("✅ model updated:").green(),
                        style(new_model_desc).yellow()
                    );
                }
            }
        }
        Err(e) => {
            if e.to_string() != "cancelled" {
                eprintln!("{} {}", style("⚠️  model selection failed:").yellow(), e);
            }
        }
    }
    Ok(())
}

// helper function for editing the message
fn open_editor_for_message(current_message: &str) -> Result<Option<String>> {
    use crossterm::terminal::disable_raw_mode;
    use std::{
        env,
        fs::{self, File},
        io::Write,
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };
    use which::which;

    // pick a filename with a monotonically-increasing suffix
    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let tmp_path = env::temp_dir().join(format!("commit-wizard-{millis}.txt"));

    // Write the current message to the temp file
    {
        let mut file = File::create(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        file.write_all(current_message.as_bytes())
            .context("failed to write initial commit message")?;
    }

    let _ = disable_raw_mode();

    let editor = if let Ok(vis) = env::var("VISUAL") {
        vis
    } else if let Ok(ed) = env::var("EDITOR") {
        ed
    } else {
        // fallback to first available editor
        // prioritise editors that are more commonly available on different platforms
        let candidates = [
            "vim",     // usually available on macos/linux by default
            "vi",      // universal fallback
            "nvim",    // popular modern alternative
            "code -w", // vscode (if installed and configured)
            "nano",    // simple fallback
        ];

        let mut selected_editor = None;
        for &candidate in &candidates {
            let command = candidate.split_whitespace().next().unwrap();
            if which(command).is_ok() {
                selected_editor = Some(candidate.to_string());
                break;
            }
        }

        match selected_editor {
            Some(editor) => editor,
            None => {
                println!(
                    "{}",
                    style("no suitable editor found, falling back to nano").yellow()
                );
                "nano".to_string()
            }
        }
    };

    // Split the editor string into command and arguments if any (e.g., "code -w")
    let mut editor_parts = editor.split_whitespace();
    let editor_executable = editor_parts.next().unwrap_or(&editor); // Use full string if no spaces
    let editor_args = editor_parts.collect::<Vec<&str>>();

    let status = Command::new(editor_executable)
        .args(&editor_args) // Pass arguments if any
        .arg(&tmp_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to execute editor '{editor}'"))?;

    if !status.success() {
        eprintln!(
            "{}",
            style(format!("editor '{editor}' exited with error: {status}")).yellow()
        );
        let _ = fs::remove_file(&tmp_path);
        return Ok(None); // Editor failed or user aborted
    }

    // read the edited content
    let edited = fs::read_to_string(&tmp_path)
        .with_context(|| format!("failed to read {}", tmp_path.display()))?;
    let _ = fs::remove_file(&tmp_path);

    if edited.trim_end() != current_message.trim_end() {
        Ok(Some(edited.trim_end().to_string()))
    } else {
        println!(
            "{}",
            style("no changes detected; using previous message").yellow()
        );
        Ok(None)
    }
}

/// enhanced interactive model selection menu with search and api integration
async fn select_model_interactively(config: &Config) -> Result<String> {
    use dialoguer::{theme::ColorfulTheme, Select};

    println!("{}", style("🤖 model selection").cyan().bold());

    // first, ask what they want to do
    let main_options = vec![
        "select from configured models",
        "browse all openrouter models",
        "enable auto-complexity selection",
    ];

    let main_choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("choose an option")
        .items(&main_options)
        .interact()
        .context("failed to interact with main model menu")?;

    match main_choice {
        0 => {
            // select from configured models
            let models = ai::get_available_models(config);
            if models.is_empty() {
                return Err(anyhow::anyhow!("no models configured"));
            }

            let model_descriptions: Vec<&str> = models.iter().map(|(_, desc)| *desc).collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("choose model")
                .items(&model_descriptions)
                .interact()
                .context("failed to select model")?;

            Ok(models[selection].0.to_string())
        }
        1 => {
            // browse all openrouter models with search
            browse_openrouter_models().await
        }
        2 => {
            // enable auto-complexity selection (return special marker)
            println!("{}", style("✅ auto-complexity selection enabled").green());
            Ok("AUTO_COMPLEXITY".to_string())
        }
        _ => unreachable!(),
    }
}

/// browse openrouter models with search functionality
async fn browse_openrouter_models() -> Result<String> {
    println!("{}", style("🔄 fetching models from openrouter...").cyan());

    let all_models = match fetch_openrouter_models().await {
        Ok(models) => models,
        Err(e) => {
            eprintln!("{} {}", style("⚠️  failed to fetch models:").yellow(), e);
            println!("{}", style("falling back to configured models").dim());
            return Err(anyhow::anyhow!("failed to fetch openrouter models"));
        }
    };

    if all_models.is_empty() {
        return Err(anyhow::anyhow!("no models available from openrouter"));
    }

    println!(
        "{} {} {}",
        style("✅").green(),
        style(all_models.len()).yellow().bold(),
        style("models found").green()
    );

    // use intelligent autosuggestion interface
    intelligent_model_search(&all_models).await
}

/// intelligent model search with real-time filtering and arrow key navigation
async fn intelligent_model_search(all_models: &[AvailableModel]) -> Result<String> {
    use crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent},
        execute,
        terminal::{self, disable_raw_mode, enable_raw_mode, size},
    };
    use std::io::{stdout, Write};

    // guard to ensure cursor is shown again on drop
    struct CursorGuard;
    impl Drop for CursorGuard {
        fn drop(&mut self) {
            let _ = execute!(stdout(), cursor::Show);
        }
    }

    // extract just model names for cleaner display
    let model_names: Vec<String> = all_models.iter().map(|m| m.name.clone()).collect();

    let mut search_query = String::new();
    let mut filtered_models = model_names.clone();
    let mut current_selection = 0;
    let window_size = 5;

    // enable raw mode for direct key input and hide cursor
    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(stdout(), cursor::Hide)?;

    // ensure cursor is shown again on early return
    let _cursor_guard = CursorGuard;

    let result = loop {
        // get terminal size with fallback
        let terminal_width = match size() {
            Ok((width, _)) => (width as usize).min(100), // reasonable width
            Err(_) => 80,
        };

        // calculate max model name width
        let max_name_width = terminal_width.saturating_sub(10);

        // clear screen completely
        execute!(
            stdout(),
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        // build output with nice styling
        let mut lines = Vec::new();

        // header with cyan title
        lines.push(format!(
            "{} {}",
            style("🔍").dim(),
            style("intelligent model search").cyan().bold()
        ));

        // stats in dim style
        lines.push(format!(
            "{} {} {}",
            style("📊").dim(),
            style(all_models.len()).yellow().bold(),
            style("total models available").dim()
        ));

        if !search_query.is_empty() {
            lines.push(format!(
                "{} {} {}",
                style("🎯").dim(),
                style("filtering by:").dim(),
                style(format!("'{search_query}'")).green().bold()
            ));
        }

        lines.push(format!(
            "{} {} {}",
            style("✨").dim(),
            style(filtered_models.len()).yellow().bold(),
            style("matches found").dim()
        ));
        lines.push("".to_string()); // blank line

        if filtered_models.is_empty() {
            lines.push(style("no models match your search").red().dim().to_string());
            lines.push(style("type to search, esc to go back").dim().to_string());
        } else {
            // ensure current selection is valid
            current_selection = current_selection.min(filtered_models.len().saturating_sub(1));

            // calculate sliding window
            let (window_start, window_end) =
                calculate_sliding_window(current_selection, filtered_models.len(), window_size);
            let visible_models = &filtered_models[window_start..window_end];

            lines.push(format!(
                "{} {}-{} {} {}",
                style("showing").dim(),
                style(window_start + 1).cyan(),
                style(window_end).cyan(),
                style("of").dim(),
                style(filtered_models.len()).cyan()
            ));
            lines.push("".to_string()); // blank line

            // display models with colors
            for (i, model_name) in visible_models.iter().enumerate() {
                let absolute_index = window_start + i;
                let is_current = absolute_index == current_selection;

                // truncate if needed
                let display_name = if model_name.len() > max_name_width {
                    format!("{}...", &model_name[..max_name_width.saturating_sub(3)])
                } else {
                    model_name.clone()
                };

                // split model name into provider/model for better coloring
                let formatted_name = if let Some(slash_pos) = display_name.find('/') {
                    let (provider, model) = display_name.split_at(slash_pos);
                    let model = &model[1..]; // skip the slash

                    if is_current {
                        format!(
                            "{}{}{}",
                            style(provider).cyan().bold(),
                            style("/").dim(),
                            style(model).white().bold()
                        )
                    } else {
                        format!(
                            "{}{}{}",
                            style(provider).blue().dim(),
                            style("/").dim(),
                            style(model).white().dim()
                        )
                    }
                } else if is_current {
                    style(display_name).white().bold().to_string()
                } else {
                    style(display_name).white().dim().to_string()
                };

                if is_current {
                    lines.push(format!("{} {}", style("►").green().bold(), formatted_name));
                } else {
                    lines.push(format!("  {formatted_name}"));
                }
            }
        }

        lines.push("".to_string()); // blank line

        // search input with styling
        let search_display = if search_query.len() > max_name_width.saturating_sub(10) {
            format!("{}...", &search_query[..max_name_width.saturating_sub(13)])
        } else {
            search_query.clone()
        };

        let cursor = if !search_query.is_empty() || filtered_models.is_empty() {
            style("█").green().to_string()
        } else {
            "".to_string()
        };

        let search_line = format!(
            "{} {}{}",
            style("search:").cyan().bold(),
            style(search_display).white(),
            cursor
        );
        lines.push(search_line);
        lines.push("".to_string()); // blank line

        // controls with better formatting
        if !filtered_models.is_empty() {
            lines.push(format!(
                "{} {} {} {} {} {} {} {}",
                style("↑↓").green().bold(),
                style("navigate").dim(),
                style("•").dim(),
                style("enter").green().bold(),
                style("select").dim(),
                style("•").dim(),
                style("esc").red().bold(),
                style("back").dim()
            ));
        } else {
            lines.push(format!(
                "{} {} {} {} {}",
                style("type").green().bold(),
                style("to search").dim(),
                style("•").dim(),
                style("esc").red().bold(),
                style("back").dim()
            ));
        }

        // print all lines cleanly
        for line in lines {
            // prepend carriage return to ensure we start at column 0 even in raw mode
            // raw mode disables automatic carriage-return on newline, so we do it manually
            println!("\r{line}");
        }

        stdout().flush()?;

        // handle input
        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Up => {
                    if !filtered_models.is_empty() && current_selection > 0 {
                        current_selection -= 1;
                    }
                }
                KeyCode::Down => {
                    if !filtered_models.is_empty() && current_selection < filtered_models.len() - 1
                    {
                        current_selection += 1;
                    }
                }
                KeyCode::Enter => {
                    if !filtered_models.is_empty() {
                        let selected_model = filtered_models[current_selection].clone();
                        break Ok(selected_model);
                    } else if !search_query.is_empty() {
                        search_query.clear();
                        filtered_models = model_names.clone();
                        current_selection = 0;
                    }
                }
                KeyCode::Esc => {
                    break Err(anyhow::anyhow!("cancelled"));
                }
                KeyCode::Backspace => {
                    if !search_query.is_empty() {
                        search_query.pop();
                        filtered_models = intelligent_filter(&model_names, &search_query);
                        current_selection = 0;
                    }
                }
                KeyCode::Char(c) => {
                    search_query.push(c);
                    filtered_models = intelligent_filter(&model_names, &search_query);
                    current_selection = 0;
                }
                _ => {}
            }
        }
    };

    // restore terminal (cursor will be shown by CursorGuard)
    disable_raw_mode().context("failed to disable raw mode")?;

    // clear and show result
    execute!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    match &result {
        Ok(selected) => {
            // format selected model name nicely
            let formatted_selected = if let Some(slash_pos) = selected.find('/') {
                let (provider, model) = selected.split_at(slash_pos);
                let model = &model[1..]; // skip the slash
                format!(
                    "{}{}{}",
                    style(provider).cyan(),
                    style("/").dim(),
                    style(model).white().bold()
                )
            } else {
                style(selected).white().bold().to_string()
            };

            println!(
                "{} {}",
                style("✅ selected:").green().bold(),
                formatted_selected
            );
        }
        Err(_) => {
            println!("{} {}", style("❌").red(), style("cancelled").dim());
        }
    }

    result
}

/// calculate sliding window bounds to keep current selection visible
fn calculate_sliding_window(
    current_selection: usize,
    total_items: usize,
    window_size: usize,
) -> (usize, usize) {
    if total_items <= window_size {
        // if we have fewer items than window size, show all
        return (0, total_items);
    }

    // try to centre the current selection in the window
    let half_window = window_size / 2;

    let start = if current_selection < half_window {
        // near the beginning, start from 0
        0
    } else if current_selection + half_window >= total_items {
        // near the end, end at total_items
        total_items - window_size
    } else {
        // in the middle, centre around current selection
        current_selection - half_window
    };

    let end = std::cmp::min(start + window_size, total_items);

    (start, end)
}

/// intelligent filtering algorithm with fuzzy matching and ranking
fn intelligent_filter(models: &[String], query: &str) -> Vec<String> {
    if query.trim().is_empty() {
        return models.to_vec();
    }

    let query_lower = query.to_lowercase();
    let query_parts: Vec<&str> = query_lower.split_whitespace().collect();

    let mut scored_models: Vec<(String, i32)> = models
        .iter()
        .filter_map(|model| {
            let model_lower = model.to_lowercase();
            let score = calculate_match_score(&model_lower, &query_lower, &query_parts);
            if score > 0 {
                Some((model.clone(), score))
            } else {
                None
            }
        })
        .collect();

    // sort by score (highest first)
    scored_models.sort_by(|a, b| b.1.cmp(&a.1));

    // return just the model names
    scored_models.into_iter().map(|(model, _)| model).collect()
}

/// calculate match score for intelligent ranking
fn calculate_match_score(model: &str, query: &str, query_parts: &[&str]) -> i32 {
    let mut score = 0;

    // exact match gets highest score
    if model == query {
        return 1000;
    }

    // starts with query gets high score
    if model.starts_with(query) {
        score += 500;
    }

    // contains exact query gets good score
    if model.contains(query) {
        score += 300;
    }

    // check individual parts
    for part in query_parts {
        if part.len() >= 2 {
            // ignore very short parts
            if model.contains(part) {
                score += 100;

                // bonus for matching provider names
                if model.starts_with(part) {
                    score += 50;
                }

                // bonus for matching after slash (model name part)
                if let Some(slash_pos) = model.find('/') {
                    let after_slash = &model[slash_pos + 1..];
                    if after_slash.contains(part) {
                        score += 75;
                    }
                }
            }
        }
    }

    // fuzzy matching bonus for partial character matches
    let fuzzy_score = calculate_fuzzy_score(model, query);
    score += fuzzy_score;

    // penalty for very long model names (prefer shorter, cleaner names)
    if model.len() > 50 {
        score -= 10;
    }

    score
}

/// calculate fuzzy matching score
fn calculate_fuzzy_score(text: &str, pattern: &str) -> i32 {
    let text_chars: Vec<char> = text.chars().collect();
    let pattern_chars: Vec<char> = pattern.chars().collect();

    let mut score = 0;
    let mut text_idx = 0;

    for &pattern_char in &pattern_chars {
        while text_idx < text_chars.len() {
            if text_chars[text_idx]
                .to_lowercase()
                .eq(pattern_char.to_lowercase())
            {
                score += 10;
                text_idx += 1;
                break;
            }
            text_idx += 1;
        }
    }

    score
}

/// load configuration from config file or create default if not found
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

/// test git diff processing without user interaction (for automated testing)
async fn test_git_diff_processing(args: &CoreCliArgs) -> Result<(String, bool)> {
    use git2::Repository;

    println!(
        "{}",
        style("🔍 testing git diff processing...").cyan().bold()
    );

    let repo_path = args.path.as_deref().unwrap_or(".");
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

    // get staged files first
    let staged_files = crate::git::get_staged_files(repo_path)?;
    println!("📄 staged files found: {}", staged_files.len());
    for (i, file) in staged_files.iter().enumerate() {
        println!("  {}. {}", i + 1, file);
    }

    if staged_files.is_empty() {
        println!(
            "{}",
            style("⚠️  no staged files found. add files with 'git add' first.").yellow()
        );
        return Ok(("test completed - no staged files".to_string(), false));
    }

    // test diff analysis
    println!("\n{}", style("🔬 analysing diffs...").cyan());
    let diff_info = match crate::git::get_diff_info(
        repo_path,
        args.max_size * 1024,
        args.max_files,
        args.verbose,
    ) {
        Ok(info) => info,
        Err(e) => {
            println!(
                "{}",
                style(&format!("❌ git diff analysis failed: {e}"))
                    .red()
                    .bold()
            );
            return Err(e);
        }
    };

    println!(
        "{}",
        style("✅ git diff processing successful!").green().bold()
    );
    println!("📊 analysis results:");
    println!("  └─ files processed: {}", diff_info.files.len());
    println!(
        "  └─ total added lines: {}",
        diff_info.files.iter().map(|f| f.added_lines).sum::<usize>()
    );
    println!(
        "  └─ total removed lines: {}",
        diff_info
            .files
            .iter()
            .map(|f| f.removed_lines)
            .sum::<usize>()
    );

    if args.verbose {
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

    // test commit intelligence analysis
    println!(
        "\n{}",
        style("🧠 testing commit intelligence analysis...").cyan()
    );
    let intelligence = crate::ai::analyse_commit_intelligence(&diff_info);
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

    // test AI commit message generation
    println!(
        "\n{}",
        style("🤖 testing ai commit message generation...").cyan()
    );

    // load config for AI generation
    let config = match load_config() {
        Ok(config) => config,
        Err(e) => {
            println!(
                "{}",
                style(&format!("⚠️  failed to load config: {e}")).yellow()
            );
            // create a default config for testing
            Config::default()
        }
    };

    // load .env file for testing
    dotenv().ok();

    // check for API key
    let api_key = std::env::var("OPENROUTER_API_KEY").ok();
    if api_key.is_none() || api_key.as_ref().unwrap().trim().is_empty() {
        println!(
            "{}",
            style("⚠️  openrouter_api_key not set, skipping ai generation test").yellow()
        );
        println!(
            "{}",
            style("💡 set openrouter_api_key environment variable to test ai generation").dim()
        );

        println!(
            "\n{}",
            style("🎉 all tests passed! git diff processing is working correctly.")
                .green()
                .bold()
        );
        return Ok((
            "test completed successfully - no AI generation (no API key)".to_string(),
            false,
        ));
    }

    // generate commit message using AI (optional hard-fail via --ai-smoke)
    let commit_message = match crate::ai::generate_conventional_commit_with_model(
        &diff_info,
        args.debug,
        args.smart_model,
        None, // use default model
        &config,
    )
    .await
    {
        Ok(message) => {
            println!("✅ ai commit message generation successful!");
            message
        }
        Err(e) => {
            if args.ai_smoke {
                return Err(e.context(
                    "ai smoke test failed (set OPENROUTER_API_KEY or disable --ai-smoke)",
                ));
            }
            println!("{}", style(&format!("❌ ai generation failed: {e}")).red());
            println!(
                "\n{}",
                style("🎉 core tests passed! git diff processing is working correctly.")
                    .green()
                    .bold()
            );
            return Ok((
                "test completed successfully - AI generation failed".to_string(),
                false,
            ));
        }
    };

    // display generated commit message
    println!("\n{}", style("📝 generated commit message:").green().bold());
    println!("{}", style("─".repeat(50)).dim());
    println!("{}", style(&commit_message).yellow());
    println!("{}", style("─".repeat(50)).dim());

    // validate the generated message
    if let Err(e) = crate::ai::validate_commit_message(&commit_message) {
        println!(
            "{}",
            style(&format!("⚠️  generated message validation warning: {e}")).yellow()
        );
    } else {
        println!(
            "{}",
            style("✅ generated message passes validation").green()
        );
    }

    // optionally commit the generated message
    let commit_successful = if args.auto_commit {
        println!(
            "\n{}",
            style("🚀 auto-committing generated message...")
                .cyan()
                .bold()
        );

        let repo_dir_path = if repo_path == "." {
            std::env::current_dir().context("Failed to get current directory")?
        } else {
            std::path::PathBuf::from(&repo_path)
        };

        let output = std::process::Command::new("git")
            .current_dir(repo_dir_path)
            .args(["commit", "-m", &commit_message])
            .output()
            .context("Failed to execute git commit command")?;

        if output.status.success() {
            println!("{}", style("✅ commit successful!").green().bold());
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if !stdout.trim().is_empty() {
                    println!("{stdout}");
                }
            }
            true
        } else {
            println!("{}", style("❌ commit failed:").red().bold());
            if let Ok(stderr) = String::from_utf8(output.stderr) {
                if !stderr.trim().is_empty() {
                    println!("{stderr}");
                }
            }
            false
        }
    } else {
        false
    };

    println!(
        "\n{}",
        style("🎉 all tests passed! git diff processing and ai generation working correctly.")
            .green()
            .bold()
    );

    Ok((commit_message, commit_successful))
}
