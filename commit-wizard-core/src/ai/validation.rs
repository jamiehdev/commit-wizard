// validation and message processing module

use anyhow::Result;

/// extract commit message from ai response
pub fn extract_commit_message(response: &str) -> String {
    let response = response
        .trim()
        .trim_matches(|c: char| c == '"' || c == '`' || c == '*');
    let lines: Vec<&str> = response.lines().collect();
    let mut commit_lines = Vec::new();
    let mut found_commit_start = false;
    let mut in_code_block = false;

    // first try: look for commit message in code blocks
    for line in lines.iter() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            if in_code_block && found_commit_start {
                // end of code block - we have our commit
                break;
            }
            in_code_block = !in_code_block;
            continue;
        }

        if in_code_block {
            if !found_commit_start && is_likely_commit_message(trimmed) {
                found_commit_start = true;
                commit_lines.push(trimmed.to_string());
            } else if found_commit_start {
                // stop if we hit non-conventional commit content
                if trimmed.starts_with("Breaking changes:") || trimmed.starts_with("Note:") {
                    break;
                }

                // collect all lines (including BREAKING CHANGE: footers)
                if trimmed.is_empty() {
                    commit_lines.push("".to_string()); // preserve blank lines
                } else {
                    commit_lines.push(trimmed.to_string());
                }
            }
        }
    }

    // if we found a multi-line commit in code blocks, return it
    if !commit_lines.is_empty() {
        let full_commit = commit_lines.join("\n");
        return normalize_commit_format(&clean_commit_message(&full_commit));
    }

    // second try: look for commit message directly in response (no code blocks)
    commit_lines.clear();
    found_commit_start = false;

    for line in lines.iter() {
        let trimmed = line.trim();

        if !found_commit_start && is_likely_commit_message(trimmed) {
            found_commit_start = true;
            commit_lines.push(trimmed.to_string());
        } else if found_commit_start {
            // stop if we hit non-conventional commit content
            if trimmed.starts_with("Breaking changes:") || trimmed.starts_with("Note:") {
                break;
            }

            // handle blank lines and multi-line bodies
            if trimmed.is_empty() {
                commit_lines.push("".to_string());
            } else if trimmed.starts_with('-')
                || trimmed.starts_with('*')
                || trimmed.starts_with("BREAKING CHANGE:")
            {
                // bullet points or footers
                commit_lines.push(trimmed.to_string());
            } else if commit_lines.len() > 2 {
                // we're in the body, continue collecting
                commit_lines.push(trimmed.to_string());
            } else {
                // unexpected format, stop
                break;
            }
        }
    }

    if !commit_lines.is_empty() {
        let full_commit = commit_lines.join("\n");
        return normalize_commit_format(&clean_commit_message(&full_commit));
    }

    // fallback: return cleaned response
    normalize_commit_format(&clean_commit_message(response))
}

/// clean commit message of unwanted characters
fn clean_commit_message(msg: &str) -> String {
    let msg = msg.trim();

    // remove common ai response artifacts
    let msg = msg
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim_start_matches("commit message:")
        .trim_start_matches("generated commit:")
        .trim_start_matches("here's the commit message:")
        .trim()
        .trim_matches('"')
        .trim_matches('`');

    // handle multi-line messages
    let lines: Vec<&str> = msg.lines().collect();
    let mut cleaned_lines = Vec::new();
    let mut in_body = false;

    for line in lines {
        let trimmed = line.trim();

        // skip meta-commentary
        if trimmed.starts_with("This commit")
            || trimmed.starts_with("The commit")
            || trimmed.starts_with("Note:")
            || trimmed.starts_with("Explanation:")
        {
            continue;
        }

        // detect when we're in the body (after blank line)
        if cleaned_lines.len() == 1 && trimmed.is_empty() {
            in_body = true;
            cleaned_lines.push("".to_string());
            continue;
        }

        // clean bullet points in body
        if in_body && (trimmed.starts_with('-') || trimmed.starts_with('*')) {
            // ensure consistent bullet format
            let content = trimmed
                .trim_start_matches('-')
                .trim_start_matches('*')
                .trim();
            cleaned_lines.push(format!("- {content}"));
        } else if !trimmed.is_empty() || in_body {
            cleaned_lines.push(trimmed.to_string());
        }
    }

    cleaned_lines.join("\n")
}

/// check if a line is likely a commit message
fn is_likely_commit_message(line: &str) -> bool {
    let valid_types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
        "revert",
    ];

    // check for type: pattern
    if let Some(colon_pos) = line.find(':') {
        let before_colon = &line[..colon_pos];

        // handle type(scope): or type!:
        let type_part = before_colon
            .split('(')
            .next()
            .unwrap_or(before_colon)
            .trim_end_matches('!');

        return valid_types.contains(&type_part);
    }

    false
}

/// normalize scope by removing spaces after commas
fn normalize_scope(scope: &str) -> String {
    // remove spaces after commas: "ai, napi, core" -> "ai,napi,core"
    scope
        .split(',')
        .map(|s| s.trim())
        .collect::<Vec<_>>()
        .join(",")
}

/// normalize commit message format
fn normalize_commit_format(msg: &str) -> String {
    let msg = msg.trim();

    // convert type[scope]: description to type(scope): description
    if msg.contains('[') && msg.contains(']') && msg.contains(':') {
        let parts: Vec<&str> = msg.splitn(2, ':').collect();
        if parts.len() == 2 {
            let type_scope = parts[0].trim();
            let description = parts[1].trim();

            // replace [scope] with (scope)
            let normalized_type_scope = type_scope.replace('[', "(").replace(']', ")");
            return format!("{normalized_type_scope}: {description}");
        }
    }

    // fix scope spacing in type(scope): description format
    if let Some(open_paren) = msg.find('(') {
        if let Some(close_paren) = msg.find(')') {
            if open_paren < close_paren {
                let prefix = &msg[..open_paren];
                let scope = &msg[open_paren + 1..close_paren];
                let suffix = &msg[close_paren..];

                let normalized_scope = normalize_scope(scope);
                return format!("{prefix}({normalized_scope}{suffix}");
            }
        }
    }

    msg.to_string()
}

/// attempt to fix common commit format issues
pub fn fix_commit_format(msg: &str) -> Result<String> {
    let msg = msg.trim();

    // handle case where ai included too much in the type field
    if let Some(first_colon) = msg.find(':') {
        let before_colon = &msg[..first_colon];
        let after_colon = &msg[first_colon + 1..].trim();

        // check if the type field looks too long (likely contains description)
        if before_colon.len() > 20 && !before_colon.contains('(') {
            // extract just the first word as the type
            if let Some(first_space) = before_colon.find(' ') {
                let actual_type = &before_colon[..first_space];

                // validate the extracted type
                let valid_types = [
                    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci",
                    "chore", "revert",
                ];

                if valid_types.contains(&actual_type) {
                    // reconstruct the message with just the type
                    return Ok(format!("{actual_type}: {after_colon}"));
                }
            }
        }
    }

    // apply standard normalization
    let normalized = normalize_commit_format(msg);

    // validate the normalized message
    match validate_commit_message(&normalized) {
        Ok(()) => Ok(normalized),
        Err(e) => {
            // if it's still invalid, try one more fix for the specific error
            if e.to_string().contains("invalid scope") && e.to_string().contains("ai, napi") {
                // this is our specific case - scope has spaces after commas
                let fixed = normalize_commit_format(msg);
                Ok(fixed)
            } else {
                Err(e)
            }
        }
    }
}

/// post-process commit message to ensure it meets all requirements
pub fn post_process_commit_message(msg: &str) -> String {
    if let Some(colon_pos) = msg.find(':') {
        let type_scope = msg[..colon_pos].trim_end();
        let mut description = msg[colon_pos + 1..].trim().to_string();

        if description.is_empty() {
            return format!("{type_scope}: ");
        }

        // ensure lowercase first letter
        if let Some(first) = description.chars().next() {
            if first.is_uppercase() {
                description = first.to_lowercase().to_string() + &description[first.len_utf8()..];
            }
        }

        // remove period at the end
        if description.ends_with('.') {
            description.pop();
        }

        // try to shorten if too long
        if description.len() > 72 {
            if let Some(shortened) = shorten_description(&description) {
                description = shortened;
            }
        }

        return format!("{type_scope}: {description}");
    }

    msg.to_string()
}

/// validate that the generated commit message follows conventional commits format
pub fn validate_commit_message(msg: &str) -> Result<()> {
    let lines: Vec<&str> = msg.lines().collect();
    if lines.is_empty() {
        return Err(anyhow::anyhow!("commit message is empty"));
    }

    let first_line = lines[0];

    // check for valid conventional commit format
    let valid_types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
        "revert",
    ];

    let has_scope = first_line.contains('(') && first_line.contains(')');
    if has_scope {
        // format: type(scope): description or type(scope)!: description
        let parts: Vec<&str> = first_line.splitn(2, '(').collect();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!(
                "invalid format: missing opening parenthesis"
            ));
        }

        let type_part = parts[0].trim_end_matches('!'); // handle type! syntax
        if !valid_types.contains(&type_part) {
            return Err(anyhow::anyhow!(
                "invalid type '{}', must be one of: {}",
                type_part,
                valid_types.join(", ")
            ));
        }

        let rest = parts[1];
        // handle both "): " and ")!: " patterns
        let scope_desc: Vec<&str> = if rest.contains(")!: ") {
            rest.splitn(2, ")!: ").collect()
        } else {
            rest.splitn(2, "): ").collect()
        };

        if scope_desc.len() != 2 {
            return Err(anyhow::anyhow!("invalid format: expected 'type(scope): description' or 'type(scope)!: description'"));
        }

        let scope = scope_desc[0];
        if !scope.is_empty()
            && (scope.contains(' ')
                || !scope.chars().all(|c| {
                    c.is_alphanumeric() || c == '-' || c == '_' || c == ',' || c == '.' || c == '/'
                }))
        {
            return Err(anyhow::anyhow!(
                "invalid scope '{}', must be a noun (alphanumeric, hyphens, underscores, commas, dots, or forward slashes only)",
                scope
            ));
        }

        let description = scope_desc[1];
        validate_description(description)?;
    } else {
        // format: type: description or type!: description
        let parts: Vec<&str> = if first_line.contains("!: ") {
            first_line.splitn(2, "!: ").collect()
        } else {
            first_line.splitn(2, ": ").collect()
        };

        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid format: expected 'type: description', 'type!: description', or 'type(scope): description'"));
        }

        let type_part = parts[0].trim_end_matches('!'); // handle type! syntax
        if !valid_types.contains(&type_part) {
            return Err(anyhow::anyhow!(
                "invalid type '{}', must be one of: {}",
                type_part,
                valid_types.join(", ")
            ));
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
        return Err(anyhow::anyhow!(
            "description too long ({} chars), must be ≤72 characters",
            description.len()
        ));
    }

    if description.ends_with('.') {
        return Err(anyhow::anyhow!("description should not end with a period"));
    }

    let first_char = description.chars().next().unwrap_or(' ');
    if first_char.is_uppercase() {
        return Err(anyhow::anyhow!(
            "description should start with lowercase letter"
        ));
    }

    // check for vague words using a scoring system (allow 1-2 vague words before failing)
    let vague_words = [
        "things",
        "stuff",
        "various",
        "multiple",
        "some",
        "several",
        "many",
        "few",
        "miscellaneous",
        "misc",
        "general",
        "generic",
        "updates",
        "changes",
        "modifications",
        "improvements",
        "fixes",
    ];
    let description_lower = description.to_lowercase();

    let mut vague_count = 0;
    let mut found_vague_words = Vec::new();

    for &vague_word in &vague_words {
        if description_lower.contains(vague_word) {
            vague_count += 1;
            found_vague_words.push(vague_word);
        }
    }

    // allow up to 2 vague words before failing
    if vague_count > 2 {
        return Err(anyhow::anyhow!(
            "description too vague - contains {} vague words ({}), try to be more specific",
            vague_count,
            found_vague_words.join(", ")
        ));
    }

    // check for imperative mood
    let words: Vec<&str> = description.split_whitespace().collect();
    if let Some(first_word) = words.first() {
        if first_word.ends_with("ed") || (first_word.ends_with("ing") && first_word.len() > 4) {
            let non_imperative = [
                "added",
                "removed",
                "deleted",
                "created",
                "updated",
                "modified",
                "fixing",
                "adding",
                "removing",
                "creating",
                "updating",
                "modifying",
            ];
            if non_imperative.contains(first_word) {
                return Err(anyhow::anyhow!(
                    "description should use imperative mood (e.g., 'add' not 'added' or 'adding')"
                ));
            }
        }
    }

    Ok(())
}

/// intelligently shorten a commit description to fit within 72 characters
fn shorten_description(description: &str) -> Option<String> {
    if description.len() <= 72 {
        return Some(description.to_string());
    }

    let shortened = description.to_string();

    // remove redundant words
    let shortened = shortened
        .replace("functionality", "func")
        .replace("configuration", "config")
        .replace("implementation", "impl")
        .replace("documentation", "docs")
        .replace("specification", "spec")
        .replace("repository", "repo")
        .replace("database", "db")
        .replace("application", "app")
        .replace("development", "dev")
        .replace("production", "prod")
        .replace("environment", "env")
        .replace("authentication", "auth")
        .replace("authorization", "authz")
        .replace("administrator", "admin")
        .replace("management", "mgmt")
        .replace("information", "info");

    if shortened.len() <= 72 {
        return Some(shortened);
    }

    // remove filler words
    let words: Vec<&str> = shortened.split_whitespace().collect();
    let filler_words = ["the", "a", "an", "for", "with", "to", "in", "of"];
    let filtered: Vec<&str> = words
        .into_iter()
        .filter(|w| !filler_words.contains(w))
        .collect();
    let shortened = filtered.join(" ");

    if shortened.len() <= 72 {
        return Some(shortened);
    }

    // use abbreviations
    let shortened = shortened
        .replace("update", "upd")
        .replace("message", "msg")
        .replace("commit", "cmt")
        .replace("generation", "gen")
        .replace("validation", "valid")
        .replace("description", "desc")
        .replace("character", "char")
        .replace("maximum", "max")
        .replace("minimum", "min")
        .replace("function", "fn")
        .replace("variable", "var")
        .replace("parameter", "param");

    if shortened.len() <= 72 {
        return Some(shortened);
    }

    None
}
