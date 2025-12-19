use std::collections::{HashMap, HashSet};

use crate::git::DiffInfo;

#[derive(Debug, Clone)]
struct CandidateScore {
    count: usize,
    depth_total: usize,
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
    let total_files = diff_info.files.len().max(1);
    let mut scores: HashMap<String, CandidateScore> = HashMap::new();

    for file in &diff_info.files {
        let mut seen_for_file: HashSet<String> = HashSet::new();
        let parts: Vec<&str> = file.path.split('/').collect();

        if parts.len() < 2 {
            continue;
        }


        for (idx, part) in parts.iter().enumerate().take(parts.len().saturating_sub(1)) {
            let depth = idx + 1;
            if depth > 3 {
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
            });
            entry.count += 1;
            entry.depth_total += depth;
        }
    }

    let mut ranked: Vec<(String, i32)> = scores
        .into_iter()
        .map(|(name, score)| {
            let coverage = score.count as f32 / total_files as f32;
            let mut points = (score.count as i32) * 10 + (score.depth_total as i32) * 2;

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

    cleaned
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
        };

        let candidates = derive_scope_candidates(&diff_info);

        assert!(!candidates.contains(&"src".to_string()));
        assert!(candidates.contains(&"commit-wizard-core".to_string()));
        assert!(candidates.contains(&"commit-wizard-cli".to_string()));
        assert!(candidates.contains(&"core".to_string()));
        assert!(candidates.contains(&"cli".to_string()));
    }
}
