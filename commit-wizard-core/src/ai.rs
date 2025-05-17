use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use crate::git::DiffInfo; // this will be local to commit-wizard-core

// ... (rest of the content from src/ai.rs) ...
// (Identical content as read previously)
// structure for openrouter api request
#[derive(Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

// structure for openrouter api response
#[derive(Deserialize)]
struct OpenRouterResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

/// generate a conventional commit message based on the diff information
pub async fn generate_conventional_commit(diff_info: &DiffInfo) -> Result<String> {
    // get api key and model from environment variables
    let api_key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;
    
    let model = env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| "nvidia/llama-3.1-nemotron-ultra-253b-v1:free".to_string());
    
    // construct the prompt for the ai
    let prompt = construct_prompt(diff_info);
    
    // create a new progress bar for the API call
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&[
                "🧙 ⠋", "🧙 ⠙", "🧙 ⠹",
                "🧙 ⠸", "🧙 ⠼", "🧙 ⠴",
                "🧙 ⠦", "🧙 ⠧", "🧙 ⠇",
                "🧙 ⠏"
            ])
            .template("{spinner} generating...")
            .unwrap()
    );
    spinner.enable_steady_tick(Duration::from_millis(120));
    
    // prepare request to openrouter api
    let client = reqwest::Client::new();
    let request = OpenRouterRequest {
        model,
        messages: vec![
            Message {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            Message {
                role: "user".to_string(),
                content: prompt,
            },
        ],
    };
    
    // send request to openrouter api in a wrapped block to ensure spinner is cleaned up
    let result = async {
        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to send request to openrouter api")?;
        
        // parse response
        let response_body = response
            .json::<OpenRouterResponse>()
            .await
            .context("failed to parse openrouter api response")?;
        
        // extract and return the generated commit message
        match response_body.choices.first() {
            Some(choice) => Ok(choice.message.content.trim().to_string()),
            None => Err(anyhow::anyhow!("no response from openrouter api")),
        }
    }.await;
    
    // stop and clear the spinner
    spinner.finish_and_clear();
    
    result
}

/// construct a prompt for the ai model based on the diff information
fn construct_prompt(diff_info: &DiffInfo) -> String {
    // start with a summary of changes
    let mut prompt = format!("generate a conventional commit message for the following changes:\n\n");
    prompt.push_str(&diff_info.summary);
    prompt.push_str("\n\n");
    
    // add diff content for each file (limited to avoid huge prompts)
    prompt.push_str("detailed changes:\n\n");
    
    for file in &diff_info.files {
        prompt.push_str(&format!("file: {}\n", file.path));
        prompt.push_str(&format!("added lines: {}, removed lines: {}\n", 
                               file.added_lines, file.removed_lines));
        
        // truncate diff content if it's too long
        let diff_content = if file.diff_content.len() > 1000 {
            format!("{}... (truncated)", &file.diff_content[..1000])
        } else {
            file.diff_content.clone()
        };
        
        prompt.push_str(&format!("diff:\n{}\n\n", diff_content));
    }
    
    prompt.push_str("\nplease generate a conventional commit message for these changes following the conventional commits specification (https://www.conventionalcommits.org/). the message should be in the format:\n\n<type>[optional scope]: <description>\n\n[optional body]\n\n[optional footer(s)]\n\nwhere type is one of: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert.\n\nimportant: do not use backticks, markdown formatting, or any special characters that might cause problems in shell commands. use plain text only. do not add ticket references like 'refs: #123' or 'closes: #123' unless they are explicitly mentioned in the diff.\n\nonly include the final commit message without any explanations or extra text.");
    
    prompt
}

// system prompt that guides the ai in generating conventional commit messages
const SYSTEM_PROMPT: &str = r#"you are commitwizard, an expert at creating conventional commit messages for git commits.

your task is to generate a well-formatted conventional commit message based on git diff information.

the commit message should follow the conventional commits specification (https://www.conventionalcommits.org/):

<type>[optional scope]: <description>

[optional body]

[optional footer(s)]

where:
- type: must be one of: feat (new feature), fix (bug fix), docs (documentation), style (formatting), refactor (code restructuring), perf (performance), test (tests), build (build system), ci (CI), chore (maintenance), revert (revert previous commit)
- scope: optional, can be anything specifying the section of the codebase
- description: a short summary of the code changes, in present tense, not capitalised, and no period at the end
- body: optional, providing additional contextual information about the changes. always use hyphens (-) to format bullet points in the body, with each point on a new line. capitalise the first word of each bullet point
- footer: optional for breaking changes only (e.g., BREAKING CHANGE: description). do not add ticket references like "closes: #123" or "refs: #123" unless they are explicitly mentioned in the diff

guidelines:
1. use UK english spelling
2. keep the description concise (less than 72 characters)
3. use the imperative, present tense: "change" not "changed" or "changes"
4. don't capitalise the description
5. no period (.) at the end of the description
6. if there are breaking changes, use BREAKING CHANGE in the footer or add a ! after the type/scope
7. include relevant scope if it helps clarify the affected code section
8. do not use backticks (`) or any markdown formatting in the commit message
9. use plain text only, as markdown is not properly supported in git commit -m commands
10. do not add references to tickets (like 'refs: #123' or 'closes: #123') unless explicitly mentioned in the diff
11. format all bullet points in the body with hyphens (-) at the start of each line and capitalise the first word (e.g., "- Clarify" not "- clarify")

only output the commit message itself without any explanations."#; 