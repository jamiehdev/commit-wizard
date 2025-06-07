use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use crate::git::{DiffInfo, FileType, ChangeHint}; // this will be local to commit-wizard-core

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
pub async fn generate_conventional_commit(diff_info: &DiffInfo, debug: bool) -> Result<String> {
    // get api key and model from environment variables
    let api_key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;
    
    let model = env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| "deepseek/deepseek-r1:free".to_string());
    
    // construct the prompt for the ai
    let prompt = construct_prompt(diff_info);
    
    if debug {
        println!("\DEBUG: Prompt being sent to AI:");
        println!("═══════════════════════════════════════");
        println!("{}", prompt);
        println!("═══════════════════════════════════════");
        println!("Prompt length: {} characters", prompt.len());
        println!("Model: {}", model);
        println!();
    }
    
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
        
        // extract and validate the generated commit message
        match response_body.choices.first() {
            Some(choice) => {
                let commit_msg = choice.message.content.trim().to_string();
                
                if debug {
                    println!("\n🐛 DEBUG: Raw AI response:");
                    println!("═══════════════════════════════════════");
                    println!("{}", commit_msg);
                    println!("═══════════════════════════════════════");
                    println!("Length: {} characters", commit_msg.len());
                    println!("First line: {:?}", commit_msg.lines().next().unwrap_or(""));
                    println!("Total lines: {}", commit_msg.lines().count());
                    println!();
                }
                
                validate_commit_message(&commit_msg)?;
                Ok(commit_msg)
            },
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
    
    // add context analysis
    prompt.push_str("context analysis:\n");
    let context = analyze_commit_context(diff_info);
    prompt.push_str(&context);
    prompt.push_str("\n\n");
    
    // add diff content for each file (limited to avoid huge prompts)
    prompt.push_str("detailed changes:\n\n");
    
    for file in &diff_info.files {
        prompt.push_str(&format!("file: {} (type: {:?})\n", file.path, file.file_type));
        prompt.push_str(&format!("added lines: {}, removed lines: {}\n", 
                               file.added_lines, file.removed_lines));
        
        if !file.change_hints.is_empty() {
            prompt.push_str(&format!("change indicators: {:?}\n", file.change_hints));
        }
        
        // truncate diff content if it's too long with Unicode-safe slicing
        let diff_content = if file.diff_content.len() > 1000 {
            // Use Unicode-safe truncation to avoid panics with emoji characters
            let mut end_pos = std::cmp::min(1000, file.diff_content.len());
            
            // Find the nearest character boundary before 1000
            while end_pos > 0 && !file.diff_content.is_char_boundary(end_pos) {
                end_pos -= 1;
            }
            
            format!("{}... (truncated)", &file.diff_content[..end_pos])
        } else {
            file.diff_content.clone()
        };
        
        prompt.push_str(&format!("diff:\n{}\n\n", diff_content));
    }
    
    prompt.push_str("\nSTRICT INSTRUCTIONS:\n1. follow the context analysis above for type and scope suggestions\n2. use the exact format: <type>[scope]: <description>\n3. if scope is suggested, use it unless clearly wrong\n4. if multiple scopes detected, omit scope entirely\n5. prioritise suggested type from context analysis\n6. description must be imperative, lowercase, under 72 chars, no period\n7. add body only if significant complexity needs explanation\n8. use UK english spelling\n9. output ONLY the commit message, no explanations\n\ngenerate the conventional commit message now:");
    
    prompt
}

/// analyze commit context to suggest appropriate conventional commit type and scope
fn analyze_commit_context(diff_info: &DiffInfo) -> String {
    let mut context = String::new();
    
    // analyze file types
    let mut file_type_counts = std::collections::HashMap::new();
    for file in &diff_info.files {
        *file_type_counts.entry(&file.file_type).or_insert(0) += 1;
    }
    
    context.push_str("file types affected: ");
    for (file_type, count) in &file_type_counts {
        context.push_str(&format!("{:?} ({}), ", file_type, count));
    }
    context.push('\n');
    
    // analyze change hints
    let mut all_hints = Vec::new();
    for file in &diff_info.files {
        all_hints.extend(file.change_hints.iter());
    }
    
    let mut hint_counts = std::collections::HashMap::new();
    for hint in &all_hints {
        *hint_counts.entry(*hint).or_insert(0) += 1;
    }
    
    if !hint_counts.is_empty() {
        context.push_str("change patterns detected: ");
        for (hint, count) in &hint_counts {
            context.push_str(&format!("{:?} ({}), ", hint, count));
        }
        context.push('\n');
    }
    
    // detect appropriate scope based on file paths
    let suggested_scope = detect_project_scope(diff_info);
    if !suggested_scope.is_empty() {
        context.push_str(&format!("suggested scope: {} (based on affected files)\n", suggested_scope));
    }
    
    // suggest commit type based on improved analysis
    context.push_str("suggested commit type: ");
    
    // prioritize structural additions over other indicators
    if hint_counts.contains_key(&ChangeHint::NewStruct) || 
       hint_counts.contains_key(&ChangeHint::NewEnum) ||
       hint_counts.contains_key(&ChangeHint::NewModule) ||
       hint_counts.contains_key(&ChangeHint::MajorAddition) {
        context.push_str("feat (major new functionality detected - new structs/enums/modules)");
    } else if hint_counts.contains_key(&ChangeHint::BugFix) && 
              !hint_counts.contains_key(&ChangeHint::NewFeature) {
        context.push_str("fix (bug fixes detected without major new functionality)");
    } else if file_type_counts.contains_key(&FileType::Test) && file_type_counts.len() == 1 {
        context.push_str("test (only test files modified)");
    } else if file_type_counts.contains_key(&FileType::Documentation) && file_type_counts.len() == 1 {
        context.push_str("docs (only documentation files modified)");
    } else if hint_counts.contains_key(&ChangeHint::Dependencies) && 
              !hint_counts.contains_key(&ChangeHint::MajorAddition) {
        context.push_str("build (dependency changes without major code additions)");
    } else if hint_counts.contains_key(&ChangeHint::Refactor) && 
              !hint_counts.contains_key(&ChangeHint::NewFeature) {
        context.push_str("refactor (code restructuring without new functionality)");
    } else if hint_counts.contains_key(&ChangeHint::Performance) {
        context.push_str("perf (performance improvements detected)");
    } else if hint_counts.contains_key(&ChangeHint::MinorTweak) && 
              !hint_counts.contains_key(&ChangeHint::NewFeature) {
        context.push_str("style or chore (minor tweaks without functional changes)");
    } else if hint_counts.contains_key(&ChangeHint::NewFunction) ||
              hint_counts.contains_key(&ChangeHint::NewFeature) {
        context.push_str("feat (new functionality detected)");
    } else {
        context.push_str("feat (default for new functionality)");
    }
    
    context
}

/// detect appropriate scope based on project structure and affected files with improved logic
fn detect_project_scope(diff_info: &DiffInfo) -> String {
    let mut scope_counts = std::collections::HashMap::new();
    let mut has_actual_dependency_changes = false;
    
    for file in &diff_info.files {
        let path = &file.path;
        
        // first check if this is ACTUALLY a dependency change
        if path == "Cargo.toml" || path == "package.json" || path == "Cargo.lock" || path == "package-lock.json" {
            // check if the diff content shows actual dependency changes
            if file.diff_content.contains("dependencies") || 
               file.diff_content.contains("version") ||
               file.diff_content.contains("\"name\":") {
                has_actual_dependency_changes = true;
            }
        }
        
        // detect scope based on directory structure and content (language-agnostic)
        let scope = if path.contains("/api/") || path.contains("/controllers/") || path.contains("/services/") {
            "api"
        } else if path.contains("/ui/") || path.contains("/components/") || path.contains("/pages/") || 
                  path.contains("/views/") || path.ends_with(".cshtml") || path.ends_with(".razor") {
            "ui"
        } else if path.contains("/models/") || path.contains("/entities/") || path.contains("/dto/") {
            "models"
        } else if path.contains("/auth/") || path.contains("/security/") || path.contains("/identity/") {
            "auth"
        } else if path.contains("/database/") || path.contains("/migrations/") || path.contains("/data/") {
            "db"
        } else if path.contains("/config/") || path.contains("/settings/") || file.file_type == crate::git::FileType::Config {
            "config"
        } else if path.ends_with(".css") || path.ends_with(".scss") || path.ends_with(".sass") || 
                  path.ends_with(".less") || path.contains("/styles/") || path.contains("/css/") {
            "styles"
        } else if path.ends_with(".js") || path.ends_with(".ts") || path.ends_with(".jsx") || 
                  path.ends_with(".tsx") || path.contains("/scripts/") || path.contains("/js/") {
            "scripts"
        } else if path.ends_with(".cs") && !path.contains("test") {
            "core"
        } else if file.file_type == crate::git::FileType::Test {
            "test"
        } else if file.file_type == crate::git::FileType::Documentation {
            "docs"
        } else if file.file_type == crate::git::FileType::Build && has_actual_dependency_changes {
            "build"
        } else if path.contains(".github/") || path.contains("ci/") || path.contains(".yml") {
            "ci"
        } else {
            ""
        };
        
        if !scope.is_empty() {
            *scope_counts.entry(scope).or_insert(0) += 1;
        }
    }
    
    // special case: if we have dependency changes but also significant code changes,
    // prioritize the code changes scope
    let has_major_code_changes = diff_info.files.iter().any(|f| 
        f.change_hints.contains(&crate::git::ChangeHint::MajorAddition) ||
        f.change_hints.contains(&crate::git::ChangeHint::NewFeature) ||
        f.change_hints.contains(&crate::git::ChangeHint::NewStruct) ||
        f.change_hints.contains(&crate::git::ChangeHint::NewEnum)
    );
    
    if has_major_code_changes && scope_counts.contains_key("build") {
        scope_counts.remove("build"); // remove build scope when major code changes present
    }
    
    // return the most common scope, or empty if mixed/unclear
    if scope_counts.len() == 1 {
        scope_counts.keys().next().unwrap().to_string()
    } else if scope_counts.len() > 1 {
        // prioritize certain scopes over others (updated for web/enterprise patterns)
        let priority_order = ["api", "ui", "auth", "models", "db", "core", "scripts", "styles", "config", "test", "docs", "ci", "build"];
        
        for priority_scope in &priority_order {
            if scope_counts.contains_key(priority_scope) {
                let total_files = diff_info.files.len();
                let scope_count = scope_counts[priority_scope];
                
                // use scope if it represents at least 40% of changes (relaxed from 50%)
                if scope_count >= (total_files * 2) / 5 {
                    return priority_scope.to_string();
                }
            }
        }
        
        String::new() // mixed scopes, no clear winner
    } else {
        String::new()
    }
}

// system prompt that guides the ai in generating conventional commit messages
const SYSTEM_PROMPT: &str = r#"you are commitwizard, an expert at creating conventional commit messages for git commits.

your task is to generate a well-formatted conventional commit message based on git diff information following strict conventional commits specification.

MANDATORY FORMAT:
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]

STRICT TYPE RULES - use ONLY these types:
- feat: new feature or functionality
- fix: bug fix, error correction, or crash fix
- docs: documentation only changes
- style: formatting, missing semicolons, etc (no code change)
- refactor: code change that neither fixes bug nor adds feature
- perf: code change that improves performance
- test: adding missing tests or correcting existing tests
- build: changes to build system or external dependencies
- ci: changes to CI configuration files and scripts
- chore: other changes that don't modify src or test files
- revert: reverts a previous commit

STRICT SCOPE RULES:
- scope must be specific to affected component/module
- valid scopes: api, ui, auth, models, db, core, scripts, styles, config, test, docs, ci, build
- use NO scope if changes affect multiple components
- NEVER use generic scopes like "software", "app", "project", "code", "system"

STRICT DESCRIPTION RULES:
- max 72 characters
- imperative mood: "add" not "adds" or "added"
- lowercase first letter
- no period at end
- be specific about WHAT changed, not HOW

STRICT BODY RULES (if needed):
- separated by blank line from description
- use hyphens (-) for bullet points
- capitalise first word of each bullet point
- explain WHY the change was made
- wrap at 72 characters per line

STRICT FOOTER RULES:
- only for BREAKING CHANGES: "BREAKING CHANGE: description"
- no ticket references unless explicitly in diff

CRITICAL REQUIREMENTS:
1. follow the context analysis suggestions for type and scope
2. if scope is suggested, use it unless clearly inappropriate
3. be precise - "enhance ai prompt analysis" not "improve things"
4. prioritise suggested type over default assumptions
5. no markdown formatting, backticks, or special characters
6. UK english spelling only

output ONLY the commit message, no explanations or additional text."#;

/// validate that the generated commit message follows conventional commits format
fn validate_commit_message(msg: &str) -> Result<()> {
    let lines: Vec<&str> = msg.lines().collect();
    if lines.is_empty() {
        return Err(anyhow::anyhow!("commit message is empty"));
    }
    
    let first_line = lines[0];
    
    // check for valid conventional commit format
    let valid_types = ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"];
    let valid_scopes = ["api", "ui", "auth", "models", "db", "core", "scripts", "styles", "config", "test", "docs", "ci", "build"];
    
    // regex to match: type(scope): description or type: description
    let has_scope = first_line.contains('(') && first_line.contains(')');
    
    if has_scope {
        // format: type(scope): description
        let parts: Vec<&str> = first_line.splitn(2, '(').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid format: missing opening parenthesis"));
        }
        
        let type_part = parts[0];
        if !valid_types.contains(&type_part) {
            return Err(anyhow::anyhow!("invalid type '{}', must be one of: {}", type_part, valid_types.join(", ")));
        }
        
        let rest = parts[1];
        let scope_desc: Vec<&str> = rest.splitn(2, "): ").collect();
        if scope_desc.len() != 2 {
            return Err(anyhow::anyhow!("invalid format: expected 'type(scope): description'"));
        }
        
        let scope = scope_desc[0];
        if !scope.is_empty() && !valid_scopes.contains(&scope) {
            return Err(anyhow::anyhow!("invalid scope '{}', must be one of: {}", scope, valid_scopes.join(", ")));
        }
        
        let description = scope_desc[1];
        validate_description(description)?;
        
    } else {
        // format: type: description
        let parts: Vec<&str> = first_line.splitn(2, ": ").collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid format: expected 'type: description' or 'type(scope): description'"));
        }
        
        let type_part = parts[0];
        if !valid_types.contains(&type_part) {
            return Err(anyhow::anyhow!("invalid type '{}', must be one of: {}", type_part, valid_types.join(", ")));
        }
        
        let description = parts[1];
        validate_description(description)?;
    }
    
    Ok(())
}

/// validate the description part of the commit message
fn validate_description(description: &str) -> Result<()> {
    if description.is_empty() {
        return Err(anyhow::anyhow!("description cannot be empty"));
    }
    
    if description.len() > 72 {
        return Err(anyhow::anyhow!("description too long ({} chars), must be ≤72 characters", description.len()));
    }
    
    if description.ends_with('.') {
        return Err(anyhow::anyhow!("description should not end with a period"));
    }
    
    let first_char = description.chars().next().unwrap();
    if first_char.is_uppercase() {
        return Err(anyhow::anyhow!("description should start with lowercase letter"));
    }
    
    // check for imperative mood (basic check for common non-imperative patterns)
    if description.ends_with("ed") || description.ends_with("ing") {
        return Err(anyhow::anyhow!("description should use imperative mood (e.g., 'add' not 'added' or 'adding')"));
    }
    
    Ok(())
}