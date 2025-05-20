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
pub use std::env;
pub use std::process::Command as StdCommand; 
pub use std::time::Duration;

pub use crate::git::{DiffInfo, ModifiedFile, has_staged_changes, get_staged_files, get_diff_info};
pub use crate::ai::generate_conventional_commit;
pub use crate::utils::check_openrouter_api_key;

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
}

// the core commit generation and interaction logic
// this is similar to what was in the NAPI lib.rs, but now it's in the core lib.
pub async fn execute_commit_wizard_flow(args: CoreCliArgs) -> Result<(String, bool)> {
    let repo_path = args.path.unwrap_or_else(|| ".".to_string());

    // note: API key check and dotenv loading should ideally happen once 
    // in the final binary (CLI or NAPI wrapper), not necessarily in the core lib directly,
    // or be passed in, to make the core lib more testable and configurable.
    // for now, keeping it simple as it was.
    dotenv().ok();
    check_openrouter_api_key()?;

    // welcome banner and staged files check could also be caller's responsibility (CLI/NAPI)
    // for now, keeping them here for functional similarity to the original.
    println!("{}", style("\ncommit-wizard 🧙 (core engine)").cyan().bold());
    println!("{}\n", style("ai-powered conventional commit message generator").dim());

    match git::has_staged_changes(&repo_path) {
        Ok(has_staged) => {
            if has_staged {
                if let Ok(files) = git::get_staged_files(&repo_path) {
                    println!("{}\n", style("staged files:").cyan().bold());
                    for file in files {
                        println!("{}", style(format!("  - {}", file)).green());
                    }
                    println!();
                }
            } else {
                println!("{}\n", style("⚠️  no staged changes found, will analyse unstaged changes instead").yellow().bold());
            }
        },
        Err(e) => {
            eprintln!("{} {}", style("❌ error checking staged changes:").red().bold(), style(e).red());
            // decide if this is a fatal error for the library's contract
        }
    }

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "📊 ⠋", "📊 ⠙", "📊 ⠹", "📊 ⠸",
                "📊 ⠼", "📊 ⠴", "📊 ⠦", "📊 ⠧",
                "📊 ⠇", "📊 ⠏"
            ])
            .template("{spinner} analysing changes...")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(120));
    
    let diff_info = git::get_diff_info(&repo_path, args.max_size * 1024, args.max_files, args.verbose)
        .context("failed to get git diff information")?;
    
    spinner.finish_and_clear();
    
    if args.verbose {
        println!("found {} modified files", diff_info.files.len());
        for file in &diff_info.files {
            println!("- {} ({} lines added, {} lines removed)", 
                     file.path, file.added_lines, file.removed_lines);
        }
    }
    
    if diff_info.files.is_empty() {
        return Err(anyhow::anyhow!("no changes detected in the repository"));
    }
    
    let mut commit_message = ai::generate_conventional_commit(&diff_info)
        .await
        .context("failed to generate commit message")?;
    
    println!("\n{}\n", style("✅ generated commit message:").green().bold());
    println!("{}", style(&commit_message).yellow());
    println!();

    let mut should_commit_now = args.yes; // initialize with the --yes flag state
    let mut commit_succeeded = false; // track if commit was successful

    if !args.yes { // use args.yes for auto_commit behavior
        println!("{}", style("press ctrl+c at any time to exit").dim());
        
        loop {
            let options = &["yes, commit this message", "edit this message", "no, regenerate message"];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("what would you like to do?")
                .default(0)
                .items(options)
                .interact()?; 

            match selection {
                0 => { 
                    println!("{}", style("proceeding with commit...").green());
                    should_commit_now = true; // Set to commit
                    break;
                },
                1 => { 
                    println!("{}", style("opening editor for commit message...").cyan());
                    if let Some(edited_message) = open_editor_for_message(&commit_message)? {
                        commit_message = edited_message;
                        println!("{}", style("commit message updated").green());
                    } else {
                        println!("{}", style("edit cancelled, using previous message").yellow());
                    }
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                },
                2 => { 
                    println!("\n{}", style("regenerating...").cyan());
                    commit_message = ai::generate_conventional_commit(&diff_info)
                        .await
                        .context("failed to regenerate commit message")?;
                    println!("\n{}\n", style("✅ newly generated commit message:").green().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                },
                _ => unreachable!(),
            }
        } 
    } else {
        println!("{}", style("--yes flag detected, proceeding with generated message automatically.").green());
        // should_commit_now is already true if args.yes was true
    }
    
    if should_commit_now { // check the flag here
        println!("{}", style("executing commit command...").cyan());
        let repo_dir_path = if repo_path == "." {
            env::current_dir().context("Failed to get current directory")?
        } else {
            std::path::PathBuf::from(&repo_path)
        };
        
        let output = StdCommand::new("git")
            .current_dir(repo_dir_path)
            .args(&["commit", "-m", &commit_message])
            .output()
            .context("failed to execute git commit command")?;
        
        if output.status.success() {
            println!("{}", style("\n✅ commit successful!").green().bold());
            if let Ok(stdout) = String::from_utf8(output.stdout) {
                if !stdout.trim().is_empty() {
                    println!("{}", stdout);
                }
            }
            commit_succeeded = true; // mark commit as successful
        } else {
            eprintln!("{}", style("\n❌ commit failed:").red().bold());
            if let Ok(stderr) = String::from_utf8(output.stderr) {
                if !stderr.trim().is_empty() {
                    eprintln!("{}", stderr);
                }
            }
            return Err(anyhow::anyhow!("git commit command failed"));
        }
    }
    
    Ok((commit_message, commit_succeeded)) // return message and commit status
}

// helper function for editing the message
fn open_editor_for_message(current_message: &str) -> Result<Option<String>> {
    use std::{
        env,
        fs::{self, File},
        io::Write,
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };
use crossterm::terminal::disable_raw_mode;
use which::which;

    // pick a filename with a monotonically-increasing suffix
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)?
        .as_millis();
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
        let candidates = ["code -w", "nvim", "vim", "vi", "nano"];
        candidates
            .iter()
            .find(|&&cand| which(cand.split_whitespace().next().unwrap()).is_ok())
            .map(|&s| s.to_string())
            .unwrap_or_else(|| "nano".to_string())
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
        .with_context(|| format!("failed to execute editor '{}'", editor))?;

    if !status.success() {
        eprintln!(
            "{}",
            style(format!("editor '{}' exited with error: {}", editor, status)).yellow()
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
        println!("{}", style("no changes detected; using previous message").yellow());
        Ok(None)
    }
} 