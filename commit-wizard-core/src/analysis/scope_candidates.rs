use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use globset::Glob;
use serde::Deserialize;

use crate::git::DiffInfo;

#[derive(Debug, Clone)]
struct CandidateScore {
    count: usize,
    depth_total: usize,
    explicit_hits: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ScopeCandidateOptions {
    pub allow_aliases: bool,
}

impl Default for ScopeCandidateOptions {
    fn default() -> Self {
        Self { allow_aliases: true }
    }
}

pub fn derive_scope_candidates(diff_info: &DiffInfo) -> Vec<String> {
    derive_scope_candidates_with_options(diff_info, ScopeCandidateOptions::default())
}

pub fn derive_scope_candidates_with_options(
    diff_info: &DiffInfo,
    options: ScopeCandidateOptions,
) -> Vec<String> {
    let scope_rules = load_scope_rules(&diff_info.repo_root);
    derive_scope_candidates_with_rules(diff_info, options, scope_rules.as_ref())
}

pub fn derive_scope_candidates_with_rules(
    diff_info: &DiffInfo,
    options: ScopeCandidateOptions,
    scope_rules: Option<&ScopeRuleSet>,
) -> Vec<String> {
    let total_files = diff_info.files.len().max(1);
    let mut scores: HashMap<String, CandidateScore> = HashMap::new();

    for file in &diff_info.files {
        let mut seen_for_file: HashSet<String> = HashSet::new();
        let parts: Vec<&str> = file.path.split('/').collect();

        if parts.len() < 2 {
            continue;
        }


        if let Some(scope_rules) = scope_rules {
            if let Some(rule_scope) = scope_rules.match_scope(&file.path) {
                let candidate = normalise_scope_segment(rule_scope);
                if !candidate.is_empty() && seen_for_file.insert(candidate.clone()) {
                    let entry = scores.entry(candidate).or_insert(CandidateScore {
                        count: 0,
                        depth_total: 0,
                        explicit_hits: 0,
                    });
                    entry.count += 1;
                    entry.depth_total += 4;
                    entry.explicit_hits += 1;
                }
            }
        }

        for (idx, part) in parts.iter().enumerate().take(parts.len().saturating_sub(1)) {
            let depth = idx + 1;
            if depth > 4 {
                break;
            }

            let candidate = normalise_scope_segment(part);
            if candidate.is_empty() || is_generic_segment(&candidate) {
                continue;
            }

            if !seen_for_file.insert(candidate.clone()) {
                continue;
            }

            let entry = scores.entry(candidate).or_insert(CandidateScore {
                count: 0,
                depth_total: 0,
                explicit_hits: 0,
            });
            entry.count += 1;
            entry.depth_total += depth;
        }

        if let Some(file_stem) = Path::new(&file.path)
            .file_stem()
            .and_then(|stem| stem.to_str())
        {
            let candidate = normalise_scope_segment(file_stem);
            if !candidate.is_empty() && !is_generic_segment(&candidate) {
                if seen_for_file.insert(candidate.clone()) {
                    let entry = scores.entry(candidate).or_insert(CandidateScore {
                        count: 0,
                        depth_total: 0,
                        explicit_hits: 0,
                    });
                    entry.count += 1;
                    entry.depth_total += 4;
                }
            }
        }
    }

    let mut ranked: Vec<(String, i32)> = scores
        .into_iter()
        .map(|(name, score)| {
            let coverage = score.count as f32 / total_files as f32;
            let mut points = (score.count as i32) * 10
                + (score.depth_total as i32) * 2
                + (score.explicit_hits as i32) * 20;

            // penalty for overly broad candidates
            if coverage > 0.8 {
                points -= 8;
            } else if coverage > 0.6 {
                points -= 4;
            }

            (name, points)
        })
        .collect();

    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(8);

    let prefix_counts = if options.allow_aliases {
        compute_prefix_counts(&ranked)
    } else {
        HashMap::new()
    };
    let allow_single_alias = options.allow_aliases && ranked.len() == 1;

    let mut candidates: Vec<String> = Vec::new();
    for (name, _) in ranked {
        if candidates.len() >= 8 {
            break;
        }
        if !candidates.contains(&name) {
            candidates.push(name.clone());
        }

        if candidates.len() >= 8 || !options.allow_aliases {
            break;
        }
        if let Some(alias) = scope_alias(&name, &prefix_counts, allow_single_alias) {
            if !candidates.contains(&alias) && !is_generic_segment(&alias) {
                candidates.push(alias);
            }
        }
    }

    candidates
}

fn normalise_scope_segment(segment: &str) -> String {
    let cleaned = segment
        .trim()
        .to_lowercase()
        .replace(['_', ' '], "-");

    let has_commit_wizard_prefix = cleaned.starts_with("commit-wizard-");
    let without_prefix = cleaned
        .strip_prefix("commit-wizard-")
        .unwrap_or(&cleaned)
        .to_string();

    if has_commit_wizard_prefix && without_prefix == "core" {
        return String::new();
    }

    without_prefix
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '.')
        .collect()
}

fn is_generic_segment(segment: &str) -> bool {
    matches!(
        segment,
        "src"
            | "lib"
            | "tests"
            | "test"
            | "target"
            | "dist"
            | "build"
            | "crates"
            | "packages"
            | "node_modules"
            | "bin"
            | "docs"
            | ".github"
            | "examples"
            | "fixtures"
            | "main"
            | "mod"
            | "index"
    )
}

fn scope_alias(
    candidate: &str,
    prefix_counts: &HashMap<String, usize>,
    allow_single_alias: bool,
) -> Option<String> {
    if let Some((prefix, suffix)) = split_prefix_suffix(candidate) {
        if prefix_counts.get(&prefix).copied().unwrap_or(0) >= 2 {
            return Some(suffix);
        }
        if allow_single_alias && suffix.len() <= 12 {
            return Some(suffix);
        }
    }
    None
}

fn compute_prefix_counts(ranked: &[(String, i32)]) -> HashMap<String, usize> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for (name, _) in ranked {
        if let Some((prefix, _)) = split_prefix_suffix(name) {
            *counts.entry(prefix).or_insert(0) += 1;
        }
    }
    counts
}

fn split_prefix_suffix(name: &str) -> Option<(String, String)> {
    let idx = name.rfind('-')?;
    let prefix = name[..idx].to_string();
    let suffix = name[idx + 1..].to_string();
    if prefix.is_empty() || suffix.is_empty() {
        None
    } else {
        Some((prefix, suffix))
    }
}

#[derive(Debug, Clone)]
pub struct ScopeRuleSet {
    rules: Vec<CompiledScopeRule>,
    omit_scope_if_single: bool,
    omit_scope_if_ambiguous: bool,
}

#[derive(Debug, Clone)]
struct CompiledScopeRule {
    scope: String,
    matcher: globset::GlobMatcher,
}

#[derive(Debug, Deserialize)]
struct RepoScopeConfig {
    scopes: Option<ScopeConfig>,
}

#[derive(Debug, Deserialize)]
struct ScopeConfig {
    rules: Option<Vec<ScopeRule>>,
    omit_scope_if_single: Option<bool>,
    omit_scope_if_ambiguous: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ScopeRule {
    #[serde(rename = "match")]
    pattern: String,
    scope: String,
}

impl ScopeRuleSet {
    pub fn omit_scope_if_single(&self) -> bool {
        self.omit_scope_if_single
    }

    pub fn omit_scope_if_ambiguous(&self) -> bool {
        self.omit_scope_if_ambiguous
    }

    fn match_scope(&self, path: &str) -> Option<&str> {
        for rule in &self.rules {
            if rule.matcher.is_match(path) {
                return Some(&rule.scope);
            }
        }
        None
    }
}

pub fn load_scope_rules(repo_root: &str) -> Option<ScopeRuleSet> {
    let config_path = Path::new(repo_root).join("commit-wizard.config.toml");
    let content = fs::read_to_string(config_path).ok()?;
    let config: RepoScopeConfig = toml::from_str(&content).ok()?;
    let scopes = config.scopes?;
    let rules = scopes.rules?;

    let mut compiled_rules = Vec::new();
    for rule in rules {
        let glob = Glob::new(&rule.pattern).ok()?;
        compiled_rules.push(CompiledScopeRule {
            scope: normalise_scope_segment(&rule.scope),
            matcher: glob.compile_matcher(),
        });
    }

    if compiled_rules.is_empty() {
        return None;
    }

    Some(ScopeRuleSet {
        rules: compiled_rules,
        omit_scope_if_single: scopes.omit_scope_if_single.unwrap_or(false),
        omit_scope_if_ambiguous: scopes.omit_scope_if_ambiguous.unwrap_or(true),
    })
}

pub fn calculate_scope_coverage(diff_info: &DiffInfo, scope: &str) -> f32 {
    if diff_info.files.is_empty() {
        return 0.0;
    }

    let target = scope.to_lowercase();
    let mut matches = 0usize;

    for file in &diff_info.files {
        if file
            .path
            .split('/')
            .any(|segment| normalise_scope_segment(segment) == target)
        {
            matches += 1;
        }
    }

    matches as f32 / diff_info.files.len().max(1) as f32
}

pub fn calculate_scope_coverage_with_rules(
    diff_info: &DiffInfo,
    scope: &str,
    scope_rules: Option<&ScopeRuleSet>,
) -> f32 {
    if let Some(scope_rules) = scope_rules {
        let target = scope.to_lowercase();
        let mut matches = 0usize;

        for file in &diff_info.files {
            if let Some(rule_scope) = scope_rules.match_scope(&file.path) {
                if normalise_scope_segment(rule_scope) == target {
                    matches += 1;
                }
            }
        }

        if matches > 0 {
            return matches as f32 / diff_info.files.len().max(1) as f32;
        }
    }

    calculate_scope_coverage(diff_info, scope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::{DiffInfo, FileType, ModifiedFile};

    #[test]
    fn derive_scope_candidates_filters_generic_segments() {
        let diff_info = DiffInfo {
            files: vec![
                ModifiedFile {
                    path: "commit-wizard-core/src/ai/api.rs".to_string(),
                    added_lines: 1,
                    removed_lines: 0,
                    diff_content: "+fn example() {}".to_string(),
                    file_type: FileType::SourceCode,
                    change_hints: Vec::new(),
                    is_minified: false,
                },
                ModifiedFile {
                    path: "commit-wizard-cli/src/main.rs".to_string(),
                    added_lines: 1,
                    removed_lines: 0,
                    diff_content: "+fn main() {}".to_string(),
                    file_type: FileType::SourceCode,
                    change_hints: Vec::new(),
                    is_minified: false,
                },
            ],
            summary: String::new(),
            repo_root: "fixtures/does-not-exist".to_string(),
        };

        let candidates = derive_scope_candidates(&diff_info);

        assert!(!candidates.contains(&"src".to_string()));
        assert!(candidates.contains(&"cli".to_string()));
        assert!(candidates.contains(&"ai".to_string()));
        assert!(candidates.contains(&"api".to_string()));
    }
}
