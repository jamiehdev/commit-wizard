use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm};
use dotenv::dotenv;
use indicatif::{ProgressBar, ProgressStyle};
use std::env;
use std::time::Duration;

mod git;
mod ai;
mod utils;

#[derive(Parser)]
#[command(name = "commit-wizard")]
#[command(author = "jamiehdev")]
#[command(version = "0.2.0")]
#[command(about = "ai-powered conventional commit message generator", long_about = None)]
struct Cli {
    /// path to git repository (defaults to current directory)
    #[arg(short, long)]
    path: Option<String>,

    /// maximum file size in kb to analyse (files larger than this will be ignored)
    #[arg(short, long, default_value = "100")]
    max_size: usize,

    /// maximum number of files to analyse
    #[arg(short = 'f', long, default_value = "10")]
    max_files: usize,

    /// show detailed diff information
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // display welcome banner with lowercase aesthetic style
    println!("{}", style("\ncommit-wizard 🧙").cyan().bold());
    println!("{}\n", style("ai-powered conventional commit message generator").dim());
    
    // load environment variables from .env file if present
    dotenv().ok();
    
    // ensure api key is set
    if env::var("OPENROUTER_API_KEY").is_err() {
        println!("{}", style("error: OPENROUTER_API_KEY environment variable is not set.").red().bold());
        println!("{}", style("please set it with: export OPENROUTER_API_KEY=your-api-key").yellow());
        return Ok(());
    }

    // parse command line arguments
    let cli = Cli::parse();
    
    // get repository path (default to current directory)
    let repo_path = cli.path.unwrap_or_else(|| ".".to_string());
    
    // analyse git diff and generate commit message
    match generate_commit(&repo_path, cli.max_size, cli.max_files, cli.verbose).await {
        Ok(commit_msg) => {
            // display the command to use for committing
            println!("\n{}", style("✨ ready to commit! ✨").green().bold());
            println!("\n{}", style("run this command:").cyan());
            let git_command = format!("git commit -m \"{}\"", commit_msg.replace("\"", "\\\""));
            println!("{}\n", style(git_command).yellow().bold());
        }
        Err(e) => {
            println!("{} {}", style("❌ error:").red().bold(), style(e).red());
        }
    }

    Ok(())
}

async fn generate_commit(repo_path: &str, max_size: usize, max_files: usize, verbose: bool) -> Result<String> {
    // create a spinner for git diff analysis
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
    
    // get diff information
    let diff_info = git::get_diff_info(repo_path, max_size * 1024, max_files, verbose)
        .context("failed to get git diff information")?;
    
    // stop and clear the spinner
    spinner.finish_and_clear();
    
    if verbose {
        println!("found {} modified files", diff_info.files.len());
        for file in &diff_info.files {
            println!("- {} ({} lines added, {} lines removed)", 
                     file.path, file.added_lines, file.removed_lines);
        }
    }
    
    if diff_info.files.is_empty() {
        return Err(anyhow::anyhow!("no changes detected in the repository"));
    }
    
    // generate commit message using ai (spinner is handled inside ai module)
    let commit_message = ai::generate_conventional_commit(&diff_info)
        .await
        .context("failed to generate commit message")?;
    
    // ask user to confirm the commit message
    println!("\n{}\n", style("✅ generated commit message:").green().bold());
    println!("{}", style(&commit_message).yellow());
    println!();
    
    let confirmation = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("use this commit message?")
        .default(true)
        .show_default(true)
        .wait_for_newline(true)
        .interact()?;
    
    if !confirmation {
        // if user doesn't like the message, try again or exit
        println!("\n{}", style("regenerating...").cyan());
        return Box::pin(generate_commit(repo_path, max_size, max_files, verbose)).await;
    }
    
    Ok(commit_message)
}
