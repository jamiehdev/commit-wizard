use clap::Parser;
use console::style;
use commit_wizard_core::CoreCliArgs;

mod flow;

#[tokio::main]
async fn main() {
    let cli_args = CoreCliArgs::parse();
    match flow::run_cli(cli_args.clone()).await {
        Ok((final_commit_message, committed)) => {
            let is_headless = cli_args.headless || cli_args.headless_json;
            if !is_headless && !committed && !final_commit_message.is_empty() {
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
