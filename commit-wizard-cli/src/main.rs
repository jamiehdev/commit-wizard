use anyhow::Result;
use clap::Parser;
use commit_wizard_core::{CoreCliArgs, execute_commit_wizard_flow, style, dotenv};
use std::env; // required for env::var for API key check

#[tokio::main]
async fn main() -> Result<()> {
    // load environment variables from .env file if present
    // this is done here for the CLI version, similar to how NAPI version might handle it.
    dotenv().ok(); 

    // API key check - can be part of CLI startup
    // note: commit_wizard_core::execute_commit_wizard_flow also does this check internally for now.
    // for a cleaner design, the core lib might not do the check/printing itself but return specific errors.
    if env::var("OPENROUTER_API_KEY").is_err() {
        println!("{}", style("error: OPENROUTER_API_KEY environment variable is not set.").red().bold());
        println!("{}", style("please set it with: export OPENROUTER_API_KEY=your-api-key").yellow());
        // optionally, return Ok(()) or an error to prevent core logic from running if API key is essential.
        // for now, allowing core logic to also check and potentially fail.
    }

    // parse arguments using the shared CoreCliArgs struct from the core library
    let cli_args = CoreCliArgs::parse();

    // the actual commit message to display if not auto-committing
    let (final_commit_message, commit_was_performed) = match execute_commit_wizard_flow(cli_args.clone()).await { // clone cli_args if needed by subsequent logic
        Ok((commit_msg, committed)) => (commit_msg, committed),
        Err(e) => {
            // the core function already prints errors, but we might add context here
            eprintln!("{} {} {}", style("❌"), style("commit-wizard CLI failed:").red().bold(), style(&e).red());
            return Err(e); // propagate the error
        }
    };

    // if not auto-committing (--yes was not used), display the command to use.
    // the core function also prints this, but this is a good place for CLI-specific final instructions.
    if !commit_was_performed && !final_commit_message.is_empty() { // only show instructions if there is a commit message
        println!("\n{}", style("✨ CLI: ready to commit! ✨").green().bold());
        println!("{}", style("run this command from your terminal:").cyan());
        let git_command = format!("git commit -m \"{}\"", final_commit_message.replace("\"", "\\\""));
        println!("{}\n", style(git_command).yellow().bold());
    }

    Ok(())
} 