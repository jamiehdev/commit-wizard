use clap::Parser;
use commit_wizard_core::{execute_commit_wizard_flow, style, CoreCliArgs};

#[tokio::main]
async fn main() {
    let cli_args = CoreCliArgs::parse();
    match execute_commit_wizard_flow(cli_args.clone()).await {
        Ok((final_commit_message, committed)) => {
            if !committed && !final_commit_message.is_empty() {
                println!("\n{}", style("✨ CLI: ready to commit! ✨").green().bold());
                println!("{}", style("run this command from your terminal:").cyan());
                let git_command = format!(
                    "git commit -m \"{}\"",
                    final_commit_message.replace("\"", "\\\"")
                );
                println!("{}\n", style(git_command).yellow().bold());
            }
        }
        Err(e) => {
            eprintln!(
                "{} {} {}",
                style("❌"),
                style("commit-wizard CLI failed:").red().bold(),
                style(&e).red()
            );
            std::process::exit(1);
        }
    }
}
