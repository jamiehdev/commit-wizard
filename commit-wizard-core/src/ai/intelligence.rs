// commit intelligence analysis module

use super::patterns::{Pattern, PatternType};
use crate::git::{DiffInfo, ModifiedFile};
use std::collections::{HashMap, HashSet};

// commit intelligence structures
#[derive(Debug, Clone)]
pub struct CommitIntelligence {
    pub complexity_score: f32,
    pub requires_body: bool,
    pub detected_patterns: Vec<Pattern>,
    pub suggested_bullets: Vec<String>,
    pub commit_type_hint: String,
    pub scope_hint: Option<String>,
}

// helper structures for file analysis
struct FileAnalysis {
    new_files: Vec<String>,
    modified_files: Vec<String>,
    by_extension: HashMap<String, Vec<String>>,
    by_directory: HashMap<String, Vec<String>>,
}

struct ContentAnalysis {
    new_functions: usize,
    new_classes: usize,
    has_bug_fix_indicators: bool,
    has_performance_indicators: bool,
}

struct RefactoringSignals {
    is_refactoring: bool,
    description: String,
    files: Vec<String>,
}

/// analyse any commit and return clear intelligence
pub fn analyse_commit_intelligence(diff_info: &DiffInfo) -> CommitIntelligence {
    let mut intelligence = CommitIntelligence {
        complexity_score: 0.0,
        requires_body: false,
        detected_patterns: Vec::new(),
        suggested_bullets: Vec::new(),
        commit_type_hint: String::new(),
        scope_hint: None,
    };

    // detect all patterns in the changes
    let patterns = detect_universal_patterns(diff_info);
    intelligence.detected_patterns = patterns;

    // calculate complexity based on patterns
    intelligence.complexity_score = calculate_pattern_complexity(&intelligence.detected_patterns);

    // determine if body is needed
    intelligence.requires_body = determine_body_requirement(
        &intelligence.detected_patterns,
        intelligence.complexity_score,
        diff_info,
    );

    // generate suggested bullet points if body needed
    if intelligence.requires_body {
        intelligence.suggested_bullets =
            generate_bullet_suggestions(&intelligence.detected_patterns);
    }

    // suggest commit type and scope
    let (commit_type, scope) = suggest_commit_metadata(&intelligence.detected_patterns, diff_info);
    intelligence.commit_type_hint = commit_type;
    intelligence.scope_hint = scope;

    intelligence
}

/// detect patterns that work across any programming language
fn detect_universal_patterns(diff_info: &DiffInfo) -> Vec<Pattern> {
    let mut patterns = Vec::new();

    // analyse file metadata first
    let file_analysis = analyse_file_metadata(diff_info);

    // new file pattern
    if !file_analysis.new_files.is_empty() {
        let impact = calculate_new_file_impact(&file_analysis.new_files);
        patterns.push(Pattern {
            pattern_type: PatternType::NewFilePattern,
            description: format!(
                "{} new file{} introduced",
                file_analysis.new_files.len(),
                if file_analysis.new_files.len() > 1 {
                    "s"
                } else {
                    ""
                }
            ),
            impact,
            files_affected: file_analysis.new_files.clone(),
        });
    }

    // cross-layer changes
    let layers = detect_layers(&file_analysis);
    if layers.len() >= 2 {
        patterns.push(Pattern {
            pattern_type: PatternType::CrossLayerChange,
            description: format!(
                "changes span {} layers: {}",
                layers.len(),
                layers.join(", ")
            ),
            impact: 0.8 + (0.1 * layers.len() as f32),
            files_affected: diff_info.files.iter().map(|f| f.path.clone()).collect(),
        });
    }

    // mass modification pattern
    if diff_info.files.len() >= 5 {
        let total_changes: usize = diff_info
            .files
            .iter()
            .map(|f| f.added_lines + f.removed_lines)
            .sum();

        patterns.push(Pattern {
            pattern_type: PatternType::MassModification,
            description: format!(
                "{} files modified with {} total line changes",
                diff_info.files.len(),
                total_changes
            ),
            impact: (0.5 + (diff_info.files.len() as f32 * 0.1)).min(1.0),
            files_affected: diff_info.files.iter().map(|f| f.path.clone()).collect(),
        });
    }

    // analyse content patterns in a language-agnostic way
    for file in &diff_info.files {
        let file_patterns = analyse_file_content_universal(file);
        patterns.extend(file_patterns);
    }

    // configuration changes
    let config_files = detect_config_files(&file_analysis);
    if !config_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::ConfigurationDrift,
            description: "configuration or settings modified".to_string(),
            impact: 0.7,
            files_affected: config_files,
        });
    }

    // dependency changes
    let dep_files = detect_dependency_files(&file_analysis);
    if !dep_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::DependencyUpdate,
            description: "dependencies or packages updated".to_string(),
            impact: 0.6,
            files_affected: dep_files,
        });
    }

    // test changes
    let test_files = detect_test_files(&file_analysis);
    if !test_files.is_empty() && (test_files.len() as f32 / diff_info.files.len() as f32) > 0.3 {
        patterns.push(Pattern {
            pattern_type: PatternType::TestEvolution,
            description: format!(
                "{} test file{} modified",
                test_files.len(),
                if test_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.5,
            files_affected: test_files,
        });
    }

    // refactoring detection
    let refactoring_signals = detect_refactoring_patterns(diff_info);
    if refactoring_signals.is_refactoring {
        patterns.push(Pattern {
            pattern_type: PatternType::RefactoringPattern,
            description: refactoring_signals.description,
            impact: 0.7,
            files_affected: refactoring_signals.files,
        });
    }

    // documentation changes
    let doc_files = detect_doc_files(&file_analysis);
    if !doc_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::DocumentationUpdate,
            description: format!(
                "{} documentation file{} updated",
                doc_files.len(),
                if doc_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.4,
            files_affected: doc_files,
        });
    }

    // style changes
    if detect_style_changes(diff_info) {
        patterns.push(Pattern {
            pattern_type: PatternType::StyleNormalization,
            description: "formatting and style normalisations".to_string(),
            impact: 0.3,
            files_affected: diff_info.files.iter().map(|f| f.path.clone()).collect(),
        });
    }

    // ci changes
    let ci_files = detect_ci_files(&file_analysis);
    if !ci_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::CiChange,
            description: format!(
                "{} CI configuration file{} modified",
                ci_files.len(),
                if ci_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.5,
            files_affected: ci_files,
        });
    }

    // deprecation patterns
    let deprecation_files = detect_deprecation_patterns(diff_info);
    if !deprecation_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::Deprecation,
            description: format!(
                "{} deprecation{} detected - potential breaking changes",
                deprecation_files.len(),
                if deprecation_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.9,
            files_affected: deprecation_files,
        });
    }

    // security fix patterns
    let security_files = detect_security_patterns(diff_info);
    if !security_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::SecurityFix,
            description: format!(
                "{} security-related change{} detected",
                security_files.len(),
                if security_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.95,
            files_affected: security_files,
        });
    }

    patterns
}

/// analyse file metadata to understand overall structure
fn analyse_file_metadata(diff_info: &DiffInfo) -> FileAnalysis {
    let mut analysis = FileAnalysis {
        new_files: Vec::new(),
        modified_files: Vec::new(),
        by_extension: HashMap::new(),
        by_directory: HashMap::new(),
    };

    for file in &diff_info.files {
        // detect new files
        if file.removed_lines == 0 && file.added_lines > 10 {
            analysis.new_files.push(file.path.clone());
        } else {
            analysis.modified_files.push(file.path.clone());
        }

        // group by extension
        if let Some(ext) = file.path.split('.').next_back() {
            analysis
                .by_extension
                .entry(ext.to_string())
                .or_default()
                .push(file.path.clone());
        }

        // group by directory
        if let Some(dir) = file.path.rsplit('/').nth(1) {
            analysis
                .by_directory
                .entry(dir.to_string())
                .or_default()
                .push(file.path.clone());
        }
    }

    analysis
}

/// detect architectural layers
fn detect_layers(analysis: &FileAnalysis) -> Vec<String> {
    let mut layers = HashSet::new();

    for dir_name in analysis.by_directory.keys() {
        let dir_lower = dir_name.to_lowercase();
        if dir_lower.contains("controller")
            || dir_lower.contains("api")
            || dir_lower.contains("endpoint")
        {
            layers.insert("api".to_string());
        }
        if dir_lower.contains("service")
            || dir_lower.contains("business")
            || dir_lower.contains("domain")
        {
            layers.insert("service".to_string());
        }
        if dir_lower.contains("model")
            || dir_lower.contains("entity")
            || dir_lower.contains("schema")
        {
            layers.insert("model".to_string());
        }
        if dir_lower.contains("view") || dir_lower.contains("component") || dir_lower.contains("ui")
        {
            layers.insert("ui".to_string());
        }
        if dir_lower.contains("test") || dir_lower.contains("spec") {
            layers.insert("test".to_string());
        }
        if dir_lower.contains("config") || dir_lower.contains("settings") {
            layers.insert("configuration".to_string());
        }
    }

    layers.into_iter().collect()
}

/// analyse file content for universal patterns
fn analyse_file_content_universal(file: &ModifiedFile) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    let content_analysis = analyse_content(&file.diff_content);

    // feature addition pattern
    if content_analysis.new_functions >= 3 || content_analysis.new_classes >= 1 {
        patterns.push(Pattern {
            pattern_type: PatternType::FeatureAddition,
            description: format!(
                "significant new functionality ({} functions, {} classes)",
                content_analysis.new_functions, content_analysis.new_classes
            ),
            impact: 0.8,
            files_affected: vec![file.path.clone()],
        });
    }

    // bug fix pattern
    if content_analysis.has_bug_fix_indicators && file.removed_lines > 0 {
        patterns.push(Pattern {
            pattern_type: PatternType::BugFixPattern,
            description: "bug fix indicators detected".to_string(),
            impact: 0.6,
            files_affected: vec![file.path.clone()],
        });
    }

    // performance pattern
    if content_analysis.has_performance_indicators {
        patterns.push(Pattern {
            pattern_type: PatternType::PerformanceTuning,
            description: "performance improvements detected".to_string(),
            impact: 0.7,
            files_affected: vec![file.path.clone()],
        });
    }

    patterns
}

/// analyse content for patterns
fn analyse_content(diff_content: &str) -> ContentAnalysis {
    let mut analysis = ContentAnalysis {
        new_functions: 0,
        new_classes: 0,
        has_bug_fix_indicators: false,
        has_performance_indicators: false,
    };

    let content_lower = diff_content.to_lowercase();

    // count new functions/methods
    analysis.new_functions = diff_content
        .lines()
        .filter(|line| line.starts_with('+'))
        .filter(|line| {
            line.contains("fn ")
                || line.contains("function ")
                || line.contains("def ")
                || line.contains("public ")
                || line.contains("private ")
                || line.contains("protected ")
        })
        .count();

    // count new classes/structs
    analysis.new_classes = diff_content
        .lines()
        .filter(|line| line.starts_with('+'))
        .filter(|line| {
            line.contains("class ")
                || line.contains("struct ")
                || line.contains("interface ")
                || line.contains("enum ")
        })
        .count();

    // detect bug fix indicators
    analysis.has_bug_fix_indicators = content_lower.contains("fix")
        || content_lower.contains("bug")
        || content_lower.contains("error")
        || content_lower.contains("issue")
        || content_lower.contains("problem");

    // detect performance indicators
    analysis.has_performance_indicators = content_lower.contains("optimiz")
        || content_lower.contains("performance")
        || content_lower.contains("speed")
        || content_lower.contains("cache")
        || content_lower.contains("async");

    analysis
}

/// detect configuration files
fn detect_config_files(analysis: &FileAnalysis) -> Vec<String> {
    let config_extensions = [
        "json", "yaml", "yml", "toml", "ini", "conf", "config", "env",
    ];
    let mut config_files = Vec::new();

    for (ext, files) in &analysis.by_extension {
        if config_extensions.contains(&ext.as_str()) {
            config_files.extend(files.clone());
        }
    }

    config_files
}

/// detect dependency files
fn detect_dependency_files(analysis: &FileAnalysis) -> Vec<String> {
    let mut dep_files = Vec::new();

    for files in analysis.by_extension.values() {
        for file in files {
            let file_lower = file.to_lowercase();
            if file_lower.contains("package.json")
                || file_lower.contains("cargo.toml")
                || file_lower.contains("requirements.txt")
                || file_lower.contains("pom.xml")
                || file_lower.contains("build.gradle")
                || file_lower.contains("gemfile")
                || file_lower.contains("pipfile")
                || file_lower.ends_with(".lock")
            {
                dep_files.push(file.clone());
            }
        }
    }

    dep_files
}

/// detect test files
fn detect_test_files(analysis: &FileAnalysis) -> Vec<String> {
    let mut test_files = Vec::new();

    for file in &analysis.modified_files {
        let file_lower = file.to_lowercase();
        if file_lower.contains("test") || file_lower.contains("spec") {
            test_files.push(file.clone());
        }
    }

    test_files
}

/// detect documentation files
fn detect_doc_files(analysis: &FileAnalysis) -> Vec<String> {
    let doc_extensions = ["md", "rst", "txt", "adoc"];
    let mut doc_files = Vec::new();

    for (ext, files) in &analysis.by_extension {
        if doc_extensions.contains(&ext.as_str()) {
            doc_files.extend(files.clone());
        }
    }

    doc_files
}

/// detect ci/cd files
fn detect_ci_files(analysis: &FileAnalysis) -> Vec<String> {
    let mut ci_files = Vec::new();

    for file in analysis.modified_files.iter().chain(&analysis.new_files) {
        let file_lower = file.to_lowercase();
        if file_lower.contains(".github/workflows")
            || file_lower.contains(".gitlab-ci")
            || file_lower.contains("jenkinsfile")
            || file_lower.contains(".circleci")
            || file_lower.contains("azure-pipelines")
            || file_lower.contains(".travis")
        {
            ci_files.push(file.clone());
        }
    }

    ci_files
}

/// detect style changes
fn detect_style_changes(diff_info: &DiffInfo) -> bool {
    // simple heuristic: many small changes across multiple files
    let avg_changes_per_file = diff_info
        .files
        .iter()
        .map(|f| f.added_lines + f.removed_lines)
        .sum::<usize>() as f32
        / diff_info.files.len() as f32;

    avg_changes_per_file < 10.0 && diff_info.files.len() > 3
}

/// detect deprecation patterns
fn detect_deprecation_patterns(diff_info: &DiffInfo) -> Vec<String> {
    diff_info
        .files
        .iter()
        .filter(|file| {
            let content_lower = file.diff_content.to_lowercase();
            content_lower.contains("@deprecated")
                || content_lower.contains("deprecated")
                || (file.removed_lines > 10 && content_lower.contains("export"))
        })
        .map(|f| f.path.clone())
        .collect()
}

/// detect security patterns
fn detect_security_patterns(diff_info: &DiffInfo) -> Vec<String> {
    diff_info
        .files
        .iter()
        .filter(|file| {
            let content_lower = file.diff_content.to_lowercase();
            let path_lower = file.path.to_lowercase();

            // strong security indicators
            let strong_keywords = content_lower.contains("vulnerability")
                || content_lower.contains("exploit")
                || content_lower.contains("injection")
                || content_lower.contains("xss")
                || content_lower.contains("csrf")
                || content_lower.contains("cve-");

            // auth/crypto keywords
            let auth_crypto = content_lower.contains("authentication")
                || content_lower.contains("authorization")
                || content_lower.contains("encrypt")
                || content_lower.contains("token");

            // security-related paths
            let security_paths = path_lower.contains("/auth/") || path_lower.contains("/security/");

            // exclude false positives
            let is_test = path_lower.contains("test") || path_lower.contains("spec");
            let is_doc = path_lower.ends_with(".md");

            !is_test && !is_doc && (strong_keywords || (auth_crypto && security_paths))
        })
        .map(|f| f.path.clone())
        .collect()
}

/// detect refactoring patterns
fn detect_refactoring_patterns(diff_info: &DiffInfo) -> RefactoringSignals {
    let total_added: usize = diff_info.files.iter().map(|f| f.added_lines).sum();
    let total_removed: usize = diff_info.files.iter().map(|f| f.removed_lines).sum();

    // refactoring typically has balanced additions and removals
    let balance_ratio = if total_added > 0 {
        total_removed as f32 / total_added as f32
    } else {
        0.0
    };

    let is_refactoring = balance_ratio > 0.7 && balance_ratio < 1.3 && total_added > 50;

    RefactoringSignals {
        is_refactoring,
        description: if is_refactoring {
            "code restructuring with balanced changes".to_string()
        } else {
            String::new()
        },
        files: if is_refactoring {
            diff_info.files.iter().map(|f| f.path.clone()).collect()
        } else {
            Vec::new()
        },
    }
}

/// calculate impact of new files
fn calculate_new_file_impact(new_files: &[String]) -> f32 {
    let base_impact = 0.7;
    let increment = 0.1;
    (base_impact + (new_files.len() as f32 * increment)).min(1.0)
}

/// calculate overall complexity from patterns
pub fn calculate_pattern_complexity(patterns: &[Pattern]) -> f32 {
    let base_complexity = patterns.len() as f32 * 0.3;
    let pattern_complexity: f32 = patterns.iter().map(|p| p.impact).sum();
    ((base_complexity + pattern_complexity) / 2.0).min(5.0)
}

/// determine if commit body is required
pub fn determine_body_requirement(
    patterns: &[Pattern],
    complexity_score: f32,
    diff_info: &DiffInfo,
) -> bool {
    // always require body for complex commits
    if complexity_score >= 2.5 {
        return true;
    }

    // require body for certain pattern types
    for pattern in patterns {
        match pattern.pattern_type {
            PatternType::ArchitecturalShift
            | PatternType::CrossLayerChange
            | PatternType::SecurityFix
            | PatternType::Deprecation => return true,
            _ => {}
        }
    }

    // require body for many files
    if diff_info.files.len() >= 5 {
        return true;
    }

    // require body for large changes
    let total_changes: usize = diff_info
        .files
        .iter()
        .map(|f| f.added_lines + f.removed_lines)
        .sum();

    total_changes > 100
}

/// generate bullet point suggestions
pub fn generate_bullet_suggestions(patterns: &[Pattern]) -> Vec<String> {
    let mut suggestions = Vec::new();

    // prioritise patterns by impact
    let mut sorted_patterns = patterns.to_vec();
    sorted_patterns.sort_by(|a, b| {
        b.impact
            .partial_cmp(&a.impact)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for pattern in sorted_patterns.iter().take(5) {
        let bullet = match pattern.pattern_type {
            PatternType::NewFilePattern => {
                format!(
                    "introduce {} for enhanced functionality",
                    summarize_files(&pattern.files_affected)
                )
            }
            PatternType::CrossLayerChange => {
                "update multiple architectural layers for consistency".to_string()
            }
            PatternType::FeatureAddition => pattern.description.clone(),
            PatternType::BugFixPattern => {
                "resolve issues with error handling and edge cases".to_string()
            }
            PatternType::RefactoringPattern => {
                "improve code structure and maintainability".to_string()
            }
            PatternType::SecurityFix => {
                "address security vulnerabilities and harden defences".to_string()
            }
            _ => pattern.description.clone(),
        };
        suggestions.push(bullet);
    }

    suggestions
}

/// summarise file list
fn summarize_files(files: &[String]) -> String {
    if files.len() == 1 {
        files[0].clone()
    } else if files.len() <= 3 {
        files.join(", ")
    } else {
        format!("{} files", files.len())
    }
}

/// suggest commit type and scope
pub fn suggest_commit_metadata(
    patterns: &[Pattern],
    diff_info: &DiffInfo,
) -> (String, Option<String>) {
    // determine commit type based on patterns
    let mut type_scores: HashMap<&str, f32> = HashMap::new();

    for pattern in patterns {
        match pattern.pattern_type {
            PatternType::FeatureAddition | PatternType::NewFilePattern => {
                *type_scores.entry("feat").or_insert(0.0) += pattern.impact;
            }
            PatternType::BugFixPattern | PatternType::SecurityFix => {
                *type_scores.entry("fix").or_insert(0.0) += pattern.impact;
            }
            PatternType::RefactoringPattern => {
                *type_scores.entry("refactor").or_insert(0.0) += pattern.impact;
            }
            PatternType::DocumentationUpdate => {
                *type_scores.entry("docs").or_insert(0.0) += pattern.impact;
            }
            PatternType::TestEvolution => {
                *type_scores.entry("test").or_insert(0.0) += pattern.impact;
            }
            PatternType::PerformanceTuning => {
                *type_scores.entry("perf").or_insert(0.0) += pattern.impact;
            }
            PatternType::CiChange => {
                *type_scores.entry("ci").or_insert(0.0) += pattern.impact;
            }
            PatternType::DependencyUpdate => {
                *type_scores.entry("chore").or_insert(0.0) += pattern.impact;
            }
            PatternType::StyleNormalization => {
                *type_scores.entry("style").or_insert(0.0) += pattern.impact;
            }
            _ => {}
        }
    }

    let commit_type = type_scores
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(k, _)| k.to_string())
        .unwrap_or_else(|| "feat".to_string());

    // determine scope based on subsystem
    let scope = determine_intelligent_scope(diff_info);

    (commit_type, scope)
}

/// determine intelligent scope
fn determine_intelligent_scope(diff_info: &DiffInfo) -> Option<String> {
    // detect subsystem from file paths
    let subsystem = detect_subsystem(diff_info);
    if subsystem != "general" {
        return Some(subsystem);
    }

    // fallback: use most common directory
    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    for file in &diff_info.files {
        if let Some(dir) = file.path.split('/').next() {
            *dir_counts.entry(dir.to_string()).or_insert(0) += 1;
        }
    }

    dir_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(dir, _)| normalize_scope(dir))
}

/// detect subsystem from paths
fn detect_subsystem(diff_info: &DiffInfo) -> String {
    let paths: Vec<String> = diff_info.files.iter().map(|f| f.path.clone()).collect();

    // check for common subsystems
    if paths
        .iter()
        .any(|p| p.contains("auth") || p.contains("login"))
    {
        return "auth".to_string();
    }
    if paths
        .iter()
        .any(|p| p.contains("api") || p.contains("endpoint"))
    {
        return "api".to_string();
    }
    if paths
        .iter()
        .any(|p| p.contains("ui") || p.contains("component"))
    {
        return "ui".to_string();
    }
    if paths
        .iter()
        .any(|p| p.contains("database") || p.contains("model"))
    {
        return "database".to_string();
    }
    if paths
        .iter()
        .any(|p| p.contains("test") || p.contains("spec"))
    {
        return "test".to_string();
    }

    "general".to_string()
}

/// normalise scope string
fn normalize_scope(scope: &str) -> String {
    scope
        .to_lowercase()
        .replace(['_', ' '], "-")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}
