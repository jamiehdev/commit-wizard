use anyhow::Result;
use clap::Parser;
use commit_wizard_core::{execute_commit_wizard_flow, style, CoreCliArgs};

#[tokio::main]
async fn main() -> Result<()> {
    // API key validation is now centralised in the core library

    // parse arguments using the shared CoreCliArgs struct from the core library
    let cli_args = CoreCliArgs::parse();

    // the actual commit message to display if not auto-committing
    let (final_commit_message, commit_was_performed) =
        match execute_commit_wizard_flow(cli_args.clone()).await {
            // clone cli_args if needed by subsequent logic
            Ok((commit_msg, committed)) => (commit_msg, committed),
            Err(e) => {
                // the core function already prints errors, but we might add context here
                eprintln!(
                    "{} {} {}",
                    style("❌"),
                    style("commit-wizard CLI failed:").red().bold(),
                    style(&e).red()
                );
                return Err(e); // propagate the error
            }
        };

    // if not auto-committing (--yes was not used), display the command to use.
    // the core function also prints this, but this is a good place for CLI-specific final instructions.
    if !commit_was_performed && !final_commit_message.is_empty() {
        // only show instructions if there is a commit message
        println!("\n{}", style("✨ CLI: ready to commit! ✨").green().bold());
        println!("{}", style("run this command from your terminal:").cyan());
        let git_command = format!(
            "git commit -m \"{}\"",
            final_commit_message.replace("\"", "\\\"")
        );
        println!("{}\n", style(git_command).yellow().bold());
    }

    Ok(())
}
