#![warn(clippy::all)]

#[macro_use]
extern crate napi_derive;

use napi::Result as NapiResult;
use napi::Status;

// import from the core library
use commit_wizard_core::{
  execute_commit_wizard_flow,
  style, // for potential direct use of style in NAPI error messages
  CoreCliArgs,
  Parser, // <<< import the Parser trait
};

// removed mod ai, git, utils - they are in commit_wizard_core
// removed direct use of clap::Parser, console::style (except above), dialoguer, indicatif, etc.

// the #[derive(Parser)] struct CoreCliArgs is now imported from commit_wizard_core

#[napi(ts_args_type = "argv: string[]")]
pub async fn run_commit_wizard_cli(argv: Vec<String>) -> NapiResult<String> {
  // API key validation is now centralised in the core library

  // clap expects the first arg to be the program name.
  let mut full_argv = vec!["commit-wizard-napi".to_string()]; // dummy program name
  full_argv.extend(argv);

  let core_args = match CoreCliArgs::try_parse_from(&full_argv) {
    Ok(args) => args,
    Err(e) => {
      let err_msg = format!("argument parsing error: {e}\nensure you are passing arguments correctly. for example: commit-wizard --path . --yes");
      eprintln!("{}", style(&err_msg).red().bold());
      return Err(napi::Error::new(Status::InvalidArg, err_msg));
    }
  };

  // the core function `execute_commit_wizard_flow` handles its own printing of progress/messages.
  // we can keep some NAPI specific print statements if needed, or let core handle all.
  // println!("{}", style("\ncommit-wizard 🧙 (via npm/napi)").cyan().bold());

  match execute_commit_wizard_flow(core_args).await {
    Ok((commit_msg, _committed_status)) => {
      // the core function also handles printing the "git commit -m ..." command if not --yes.
      // so, we might not need to print it again here.
      // if we want NAPI specific final message:
      // println!("{}", style("NAPI: Commit wizard flow completed.").green());
      Ok(commit_msg) // return the generated (and possibly committed) message to JavaScript
    }
    Err(e) => {
      // the core function already prints detailed errors.
      // check if it's a validation error and provide more helpful feedback
      let error_str = e.to_string();
      let napi_err_msg = if error_str.contains("invalid scope") {
        format!("NAPI: commit message validation failed - {e}\nTip: Ensure the scope only contains alphanumeric characters, hyphens, underscores, dots, or forward slashes")
      } else if error_str.contains("invalid format") {
        format!("NAPI: commit message format error - {e}\nExpected format: type(scope): description")
      } else {
        format!("NAPI: error during commit wizard execution: {e}")
      };
      
      eprintln!("{}", style(&napi_err_msg).red().bold());
      Err(napi::Error::new(Status::GenericFailure, napi_err_msg))
    }
  }
}

#[napi]
pub fn sum(a: i32, b: i32) -> i32 {
  a + b
}

// the functions `execute_generate_commit` and `open_editor_for_message` are now part of `commit-wizard-core`
// and are called by `execute_commit_wizard_flow`.
