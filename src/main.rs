use anyhow::{Context, Result};
use clap::Parser;
use dotenv::dotenv;
use std::env;

mod git;
mod ai;
mod utils;

#[derive(Parser)]
#[command(name = "commit-wizard")]
#[command(author = "jamiehdev")]
#[command(version = "0.1.0")]
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
    // load environment variables from .env file if present
    dotenv().ok();
    
    // ensure api key is set
    if env::var("OPENROUTER_API_KEY").is_err() {
        println!("error: OPENROUTER_API_KEY environment variable is not set.");
        println!("please set it with: export OPENROUTER_API_KEY=your-api-key");
        return Ok(());
    }

    // parse command line arguments
    let cli = Cli::parse();
    
    // get repository path (default to current directory)
    let repo_path = cli.path.unwrap_or_else(|| ".".to_string());
    
    // analyse git diff and generate commit message
    match generate_commit(&repo_path, cli.max_size, cli.max_files, cli.verbose).await {
        Ok(commit_msg) => {
            println!("\n✅ generated conventional commit message:\n");
            println!("{}", commit_msg);
            println!("\nto use this commit message:");
            println!("git commit -m \"{}\"", commit_msg.replace("\"", "\\\""));
        }
        Err(e) => {
            println!("❌ error generating commit message: {}", e);
        }
    }

    Ok(())
}

async fn generate_commit(repo_path: &str, max_size: usize, max_files: usize, verbose: bool) -> Result<String> {
    // get diff information
    println!("📊 analysing git diff...");
    let diff_info = git::get_diff_info(repo_path, max_size * 1024, max_files, verbose)
        .context("failed to get git diff information")?;
    
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
    
    // generate commit message using ai
    println!("🤖 generating conventional commit message...");
    let commit_message = ai::generate_conventional_commit(&diff_info)
        .await
        .context("failed to generate commit message")?;
    
    Ok(commit_message)
}
