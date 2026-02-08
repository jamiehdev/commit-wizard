use crate::git::{DiffInfo, FileType, ModifiedFile};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct GuardrailResult {
    pub forced_type: Option<String>,
    pub suggested_type: Option<String>,
    pub text_only: bool,
    pub infra_only: bool,
    pub docs_only: bool,
}

pub fn analyse_guardrails(diff_info: &DiffInfo) -> GuardrailResult {
    if diff_info.files.is_empty() {
        return GuardrailResult {
            forced_type: None,
            suggested_type: None,
            text_only: false,
            infra_only: false,
            docs_only: false,
        };
    }

    let docs_only = diff_info
        .files
        .iter()
        .all(|file| matches!(file.file_type, FileType::Documentation));

    if docs_only {
        return GuardrailResult {
            forced_type: Some("docs".to_string()),
            suggested_type: Some("docs".to_string()),
            text_only: false,
            infra_only: false,
            docs_only: true,
        };
    }

    let infra_only = diff_info.files.iter().all(is_infra_file);
    let text_only = is_text_only_change(diff_info);

    let forced_type = if text_only || infra_only {
        Some("chore".to_string())
    } else {
        None
    };

    let suggested_type = if text_only || infra_only {
        Some("chore".to_string())
    } else {
        None
    };

    GuardrailResult {
        forced_type,
        suggested_type,
        text_only,
        infra_only,
        docs_only: false,
    }
}

pub fn determine_forced_commit_type(diff_info: &DiffInfo) -> Option<String> {
    analyse_guardrails(diff_info).forced_type
}

fn is_infra_file(file: &ModifiedFile) -> bool {
    matches!(file.file_type, FileType::Config | FileType::Build) || is_ci_path(&file.path)
}

fn is_text_only_change(diff_info: &DiffInfo) -> bool {
    let mut saw_source = false;

    for file in &diff_info.files {
        match file.file_type {
            FileType::Documentation => continue,
            FileType::SourceCode => {
                saw_source = true;
                if !is_text_only_diff(&file.diff_content) {
                    return false;
                }
            }
            _ => return false,
        }
    }

    saw_source
}

fn is_text_only_diff(diff_content: &str) -> bool {
    let mut added: HashMap<String, usize> = HashMap::new();
    let mut removed: HashMap<String, usize> = HashMap::new();
    let mut saw_change = false;

    for line in diff_content.lines() {
        if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("diff --git")
            || line.starts_with("@@")
        {
            continue;
        }

        let (kind, content) = if let Some(rest) = line.strip_prefix('+') {
            ('+', rest)
        } else if let Some(rest) = line.strip_prefix('-') {
            ('-', rest)
        } else {
            continue;
        };

        let content = content.trim();
        if content.is_empty() {
            continue;
        }

        saw_change = true;

        if is_comment_line(content) {
            continue;
        }

        let (skeleton, had_string) = normalise_string_skeleton(content);
        if !had_string {
            return false;
        }

        let entry = if kind == '+' { &mut added } else { &mut removed };
        *entry.entry(skeleton).or_insert(0) += 1;
    }

    if !saw_change {
        return false;
    }

    if added.len() != removed.len() {
        return false;
    }

    for (key, count) in &added {
        if removed.get(key).copied().unwrap_or(0) != *count {
            return false;
        }
    }
    for (key, count) in &removed {
        if added.get(key).copied().unwrap_or(0) != *count {
            return false;
        }
    }

    true
}

fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with("*/")
}

fn normalise_string_skeleton(line: &str) -> (String, bool) {
    let (stripped, had_string) = strip_string_literals(line);
    let collapsed = stripped
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    (collapsed.trim().to_string(), had_string)
}

fn strip_string_literals(line: &str) -> (String, bool) {
    let regex = string_literal_regex();
    let had_string = regex.is_match(line);
    let replaced = regex.replace_all(line, "\"\"");
    (replaced.to_string(), had_string)
}

fn string_literal_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#""(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|`(?:\\.|[^`\\])*`"#)
            .expect("valid string literal regex")
    })
}

fn is_ci_path(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    path_lower.contains(".github/workflows")
        || path_lower.contains(".gitlab-ci")
        || path_lower.contains("jenkinsfile")
        || path_lower.contains(".circleci")
        || path_lower.contains("azure-pipelines")
        || path_lower.contains(".travis")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_file(path: &str, file_type: FileType, diff_content: &str) -> ModifiedFile {
        ModifiedFile {
            path: path.to_string(),
            added_lines: 1,
            removed_lines: 0,
            diff_content: diff_content.to_string(),
            file_type,
            change_hints: Vec::new(),
            is_minified: false,
        }
    }

    #[test]
    fn forces_docs_for_doc_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file("README.md", FileType::Documentation, "+update")],
            summary: String::new(),
            repo_root: ".".to_string(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("docs".to_string())
        );
    }

    #[test]
    fn forces_chore_for_ci_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file(
                ".github/workflows/ci.yml",
                FileType::Config,
                "+name: ci",
            )],
            summary: String::new(),
            repo_root: ".".to_string(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("chore".to_string())
        );
    }

    #[test]
    fn forces_chore_for_text_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file(
                "src/lib.rs",
                FileType::SourceCode,
                "-let msg = \"old\";\n+let msg = \"new\";\n",
            )],
            summary: String::new(),
            repo_root: ".".to_string(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("chore".to_string())
        );
    }
}
