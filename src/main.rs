use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use dialoguer::{theme::ColorfulTheme, Select, Editor};
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
    
    /// automatically run the commit command when confirmed
    #[arg(short = 'y', long)]
    yes: bool,
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
    
    // check if there are staged changes
    match git::has_staged_changes(&repo_path) {
        Ok(has_staged) => {
            if has_staged {
                // display staged files
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
            println!("{} {}", style("❌ error checking staged changes:").red().bold(), style(e).red());
        }
    }
    
    // analyse git diff and generate commit message
    match generate_commit(&repo_path, cli.max_size, cli.max_files, cli.verbose, cli.yes).await {
        Ok(commit_msg) => {
            // if auto commit is not enabled, display the command to use for committing
            if !cli.yes {
                println!("\n{}", style("✨ ready to commit! ✨").green().bold());
                println!("\n{}", style("run this command:").cyan());
                let git_command = format!("git commit -m \"{}\"", commit_msg.replace("\"", "\\\""));
                println!("{}\n", style(git_command).yellow().bold());
            }
        }
        Err(e) => {
            println!("{} {}", style("❌ error:").red().bold(), style(e).red());
        }
    }

    Ok(())
}

async fn generate_commit(repo_path: &str, max_size: usize, max_files: usize, verbose: bool, auto_commit: bool) -> Result<String> {
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
    
    // provide instructions about using ctrl+c to exit
    println!("{}\n", style("press ctrl+c at any time to exit").dim());
    
    let options = &["yes, commit this message", "edit this message", "no, regenerate message"];
    
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("what would you like to do?")
        .default(0)
        .items(options)
        .interact()?;
    
    let mut commit_message = commit_message;
    
    match selection {
        0 => {
            // user chose "yes" - continue with commit
            println!("{}", style("proceeding with commit...").green());
        },
        1 => {
            // user chose "edit" - allow them to edit the message
            println!("{}", style("opening editor for commit message...").cyan());
            
            // create a temporary file with the commit message
            use std::fs;
            use std::io::Write;
            use std::path::PathBuf;
            use std::process::Command;
            
            let temp_dir = env::temp_dir();
            let temp_file_path = temp_dir.join("commit_message.txt");
            
            // write the commit message to the temp file
            let mut file = fs::File::create(&temp_file_path)?;
            write!(file, "{}", commit_message)?;
            file.flush()?;
            
            // try to find a suitable editor
            let editors = ["nano", "vim", "vi", "emacs", "gedit", "notepad"];
            let editor_env = env::var("EDITOR").unwrap_or_default();
            
            let editor_cmd = if !editor_env.is_empty() {
                editor_env
            } else {
                // find the first available editor
                let mut found_editor = String::from("nano"); // default fallback
                for editor in editors.iter() {
                    let which_result = Command::new("which")
                        .arg(editor)
                        .output();
                    
                    if let Ok(output) = which_result {
                        if output.status.success() {
                            found_editor = editor.to_string();
                            break;
                        }
                    }
                }
                found_editor
            };
            
            println!("{}", style(format!("using {} editor...", editor_cmd)).dim());
            
            // open the editor
            let status = Command::new(&editor_cmd)
                .arg(temp_file_path.to_str().unwrap())
                .status();
                
            match status {
                Ok(exit_status) => {
                    if exit_status.success() {
                        // read the edited content
                        match fs::read_to_string(&temp_file_path) {
                            Ok(edited_content) => {
                                commit_message = edited_content;
                                println!("{}", style("commit message updated").green());
                            },
                            Err(e) => {
                                println!("{} {}", style("error reading edited message:").red(), e);
                                println!("{}", style("using original message").yellow());
                            }
                        }
                    } else {
                        println!("{}", style("edit cancelled, using original message").yellow());
                    }
                },
                Err(e) => {
                    println!("{} {}", style("error launching editor:").red(), e);
                    println!("{}", style("using original message").yellow());
                }
            }
            
            // clean up the temp file
            let _ = fs::remove_file(temp_file_path);
        },
        2 => {
            // user chose "no" - regenerate the message
            println!("\n{}", style("regenerating...").cyan());
            return Box::pin(generate_commit(repo_path, max_size, max_files, verbose, auto_commit)).await;
        },
        _ => unreachable!(), // this should never happen with select
    }
    
    // if auto_commit is enabled, execute the git commit command directly
    if auto_commit {
        println!("{}", style("executing commit command...").cyan());
        
        use std::process::Command;
        
        // ensure we're in the right directory
        let repo_dir = if repo_path == "." {
            env::current_dir()?
        } else {
            std::path::PathBuf::from(&repo_path)
        };
        
        // execute the git commit command
        let output = Command::new("git")
            .current_dir(repo_dir)
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
        } else {
            println!("{}", style("\n❌ commit failed:").red().bold());
            if let Ok(stderr) = String::from_utf8(output.stderr) {
                if !stderr.trim().is_empty() {
                    println!("{}", stderr);
                }
            }
        }
    }
    
    Ok(commit_message)
}
