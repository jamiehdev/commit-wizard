use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::validation::validate_commit_message;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitClassification {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: Option<String>,
    pub breaking: bool,
    pub subject: String,
    #[serde(default)]
    pub body_bullets: Vec<String>,
    #[serde(default)]
    pub confidence: f32,
    #[serde(default)]
    pub scope_reason: Option<String>,
    #[serde(default)]
    pub scope_confidence: f32,
    #[serde(default)]
    pub alternatives: Vec<CommitAlternative>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitAlternative {
    #[serde(rename = "type")]
    pub commit_type: String,
    pub scope: Option<String>,
    pub why: String,
}

pub fn parse_commit_classification(response: &str) -> Result<CommitClassification> {
    let json_payload = extract_json_payload(response)
        .context("classifier response did not contain valid json")?;
    let mut classification: CommitClassification = serde_json::from_str(&json_payload)
        .context("failed to parse classifier json")?;

    classification.commit_type = classification.commit_type.trim().to_lowercase();
    classification.subject = classification.subject.trim().to_string();
    classification.scope = classification
        .scope
        .as_ref()
        .map(|scope| scope.trim().to_string())
        .filter(|scope| !scope.is_empty())
        .filter(|scope| {
            let lower = scope.to_lowercase();
            lower != "null" && lower != "none"
        });
    classification.body_bullets = classification
        .body_bullets
        .into_iter()
        .map(|bullet| normalise_bullet(&bullet))
        .filter(|bullet| !bullet.is_empty())
        .collect();
    classification.confidence = classification.confidence.clamp(0.0, 1.0);
    classification.scope_confidence = classification.scope_confidence.clamp(0.0, 1.0);
    classification.scope_reason = classification
        .scope_reason
        .as_ref()
        .map(|reason| reason.trim().to_string())
        .filter(|reason| !reason.is_empty());

    Ok(classification)
}

pub fn render_commit_message_from_classification(
    classification: &CommitClassification,
) -> Result<String> {
    let mut prefix = if let Some(scope) = classification.scope.as_ref() {
        format!("{}({})", classification.commit_type, scope)
    } else {
        classification.commit_type.clone()
    };

    if classification.breaking {
        prefix.push('!');
    }

    let subject = truncate_subject(classification.subject.trim(), 72);
    let mut message = format!("{prefix}: {subject}");

    if !classification.body_bullets.is_empty() {
        message.push_str("\n\n");
        for bullet in &classification.body_bullets {
            message.push_str(&format!("- {bullet}\n"));
        }
        message = message.trim_end().to_string();
    }

    if classification.breaking && classification.body_bullets.is_empty() {
        message.push_str("\n\nBREAKING CHANGE: ");
        message.push_str(&subject);
    }

    validate_commit_message(&message)?;

    Ok(message)
}

fn extract_json_payload(response: &str) -> Option<String> {
    let trimmed = response.trim();

    if let Some(code_block) = extract_code_block(trimmed) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&code_block) {
            return Some(value.to_string());
        }
    }

    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        if end > start {
            let candidate = trimmed[start..=end].trim();
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

fn extract_code_block(input: &str) -> Option<String> {
    let mut in_block = false;
    let mut lines = Vec::new();

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                break;
            }
            in_block = true;
            continue;
        }

        if in_block {
            lines.push(line);
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn normalise_bullet(bullet: &str) -> String {
    bullet
        .trim()
        .trim_start_matches('-')
        .trim_start_matches('*')
        .trim()
        .to_string()
}

fn truncate_subject(subject: &str, max_len: usize) -> String {
    let trimmed = subject.trim();
    if trimmed.len() <= max_len {
        return trimmed.to_string();
    }

    let mut result = String::new();
    for word in trimmed.split_whitespace() {
        if result.is_empty() {
            if word.len() > max_len {
                return word.chars().take(max_len).collect();
            }
            result.push_str(word);
        } else if result.len() + 1 + word.len() <= max_len {
            result.push(' ');
            result.push_str(word);
        } else {
            break;
        }
    }

    if result.is_empty() {
        trimmed.chars().take(max_len).collect()
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_commit_classification_from_code_block() {
        let response = r#"
```json
{
  "type": "feat",
  "scope": "core",
  "breaking": false,
  "subject": "add scope candidates",
  "body_bullets": ["Add scope derivation", "Expose candidates in output"],
  "confidence": 0.77,
  "scope_reason": "changes touch core analysis files only",
  "scope_confidence": 0.66,
  "alternatives": []
}
```
"#;
        let classification = parse_commit_classification(response).expect("parse json");
        assert_eq!(classification.commit_type, "feat");
        assert_eq!(classification.scope.as_deref(), Some("core"));
        assert_eq!(classification.subject, "add scope candidates");
        assert_eq!(classification.body_bullets.len(), 2);
        assert_eq!(
            classification.scope_reason.as_deref(),
            Some("changes touch core analysis files only")
        );
        assert!((classification.scope_confidence - 0.66).abs() < f32::EPSILON);
    }

    #[test]
    fn truncate_subject_limits_length() {
        let subject = "add classifier-based commit message generation with performance improvements";
        let truncated = truncate_subject(subject, 72);
        assert!(truncated.len() <= 72);
    }

    #[test]
    fn clamp_confidence_to_valid_range() {
        let response = r#"
{
  "type": "feat",
  "scope": null,
  "breaking": false,
  "subject": "add scope candidates",
  "body_bullets": [],
  "confidence": 1.6,
  "scope_reason": "single package touched",
  "scope_confidence": 1.4,
  "alternatives": []
}
"#;
        let classification = parse_commit_classification(response).expect("parse json");
        assert!((classification.confidence - 1.0).abs() < f32::EPSILON);
        assert!((classification.scope_confidence - 1.0).abs() < f32::EPSILON);
    }
}
