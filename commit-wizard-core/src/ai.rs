use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::collections::{HashMap, HashSet};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;
use regex::Regex;
use std::sync::OnceLock;
use crate::git::{DiffInfo, ModifiedFile};
use crate::Config;

// cached regex patterns for performance
static FUNCTION_REGEX: OnceLock<Regex> = OnceLock::new();
static IMPORT_REGEX: OnceLock<Regex> = OnceLock::new();

fn get_function_regex() -> &'static Regex {
    FUNCTION_REGEX.get_or_init(|| {
        Regex::new(r"(?i)(pub\s+)?(async\s+)?(fn|function|def|class|struct|enum|interface|type)\s+(\w+)").unwrap()
    })
}

fn get_import_regex() -> &'static Regex {
    IMPORT_REGEX.get_or_init(|| {
        Regex::new(r"(?i)(use|import|from)\s+([^\s;]+)").unwrap()
    })
}

// openrouter api structures
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

// universal commit intelligence structures
#[derive(Debug, Clone)]
pub struct CommitIntelligence {
    pub complexity_score: f32,
    pub requires_body: bool,
    pub detected_patterns: Vec<Pattern>,
    pub suggested_bullets: Vec<String>,
    pub commit_type_hint: String,
    pub scope_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Pattern {
    pub pattern_type: PatternType,
    pub description: String,
    pub impact: f32,
    pub files_affected: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PatternType {
    NewFilePattern,
    MassModification,
    CrossLayerChange,
    InterfaceEvolution,
    ArchitecturalShift,
    ConfigurationDrift,
    DependencyUpdate,
    RefactoringPattern,
    FeatureAddition,
    BugFixPattern,
    TestEvolution,
    DocumentationUpdate,
    StyleNormalization,
    PerformanceTuning,
    SecurityHardening,
}

// file analysis helper structures
struct FileAnalysis {
    new_files: Vec<String>,
    modified_files: Vec<String>,
    by_extension: HashMap<String, Vec<String>>,
    by_directory: HashMap<String, Vec<String>>,
}

struct ContentAnalysis {
    new_functions: usize,
    new_classes: usize,
    api_changes: usize,
    has_bug_fix_indicators: bool,
    has_performance_indicators: bool,
}

struct RefactoringSignals {
    is_refactoring: bool,
    description: String,
    files: Vec<String>,
}

/// generate a conventional commit message based on the diff information
pub async fn generate_conventional_commit(diff_info: &DiffInfo, debug: bool, smart_model: bool, config: &Config) -> Result<String> {
    generate_conventional_commit_with_model(diff_info, debug, smart_model, None, config).await
}

/// generate a conventional commit message with optional custom model
pub async fn generate_conventional_commit_with_model(
    diff_info: &DiffInfo, 
    debug: bool, 
    smart_model: bool, 
    custom_model: Option<String>, 
    config: &Config
) -> Result<String> {
    // start spinner immediately to show activity
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")
            .unwrap()
    );
    spinner.set_message("🧙 analysing commit changes...");
    spinner.enable_steady_tick(Duration::from_millis(80));
    
    // analyse commit intelligence (this is the expensive operation)
    let intelligence = analyse_commit_intelligence(diff_info);
    
    // select model based on complexity or custom choice
    let model = if let Some(custom) = custom_model {
        custom
    } else if smart_model {
        select_model_for_complexity(&intelligence, debug, config)
    } else {
        config.models.default.clone()
    };
    
    if debug {
        println!("🤖 selected model:\n{}", model);
        println!();
    }
    
    // update spinner message
    spinner.set_message(format!("🧙 generating commit message with {}...", model));
    
    // construct intelligent prompt
    let prompt = construct_intelligent_prompt(diff_info, &intelligence);
    
    if debug {
        println!("🐛 DEBUG: Commit intelligence analysis:");
        println!("  └─ Complexity score: {:.1}/5.0", intelligence.complexity_score);
        println!("  └─ Requires body: {}", intelligence.requires_body);
        println!("  └─ Detected patterns: {}", intelligence.detected_patterns.len());
        for pattern in &intelligence.detected_patterns {
            println!("     • {}: {} (impact: {:.1})", 
                format_pattern_type(&pattern.pattern_type),
                pattern.description,
                pattern.impact
            );
        }
        println!("  └─ Suggested type: {}", intelligence.commit_type_hint);
        if let Some(scope) = &intelligence.scope_hint {
            println!("  └─ Suggested scope: {}", scope);
        }
        println!();
        
        println!("🐛 DEBUG: File analysis summary:");
        for (i, file) in diff_info.files.iter().enumerate() {
            if i >= 3 { 
                println!("  ... and {} more files", diff_info.files.len() - i);
                break; 
            }
            println!("  └─ {}: +{} -{} lines", file.path, file.added_lines, file.removed_lines);
            let changes = analyse_file_changes_for_prompt(&file.diff_content);
            if !changes.is_empty() {
                for change in &changes {
                    println!("     • {}", change);
                }
            }
        }
        println!();
        
        println!("🐛 DEBUG: Full prompt being sent to AI:");
        println!("═══════════════════════════════════════");
        println!("{}", prompt);
        println!("═══════════════════════════════════════");
        println!();
    }
    
    let api_key = env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;
    
    let max_retries = 3;
    let mut retry_count = 0;
    
    let result = loop {
        let current_prompt = if retry_count > 0 {
            format!("{}\n\nIMPORTANT: The description MUST be under 72 characters. Be concise!", prompt)
        } else {
            prompt.clone()
        };
        
        let request = OpenRouterRequest {
            model: model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: get_system_prompt(&intelligence).to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: current_prompt,
                },
            ],
        };
        
        let response = match make_api_request(&api_key, request).await {
            Ok(resp) => resp,
            Err(e) => break Err(e),
        };
        
        match response.choices.first() {
            Some(choice) => {
                let raw_response = &choice.message.content;
                
                if debug {
                    println!("🐛 DEBUG: Raw API response:");
                    println!("═══════════════════════════════════════");
                    println!("{}", raw_response);
                    println!("═══════════════════════════════════════");
                    println!();
                }
                
                let commit_msg = extract_commit_message(&raw_response);
                let commit_msg = post_process_commit_message(&commit_msg);
                
                if debug {
                    println!("🐛 DEBUG: Extracted and processed commit message:");
                    println!("═══════════════════════════════════════");
                    println!("'{}'", commit_msg);
                    println!("═══════════════════════════════════════");
                    println!();
                }
                
                match validate_commit_message(&commit_msg) {
                    Ok(()) => break Ok(commit_msg),
                    Err(e) => {
                        if e.to_string().contains("description too long") && retry_count < max_retries {
                            retry_count += 1;
                            if debug {
                                println!("⚠️  description too long, retrying ({}/{})", retry_count, max_retries);
                            }
                            continue;
                        } else {
                            break Err(e);
                        }
                    }
                }
            },
            None => break Err(anyhow::anyhow!("no response from openrouter api")),
        }
    };
    
    spinner.finish_and_clear();
    result
}

/// make api request to openrouter
async fn make_api_request(api_key: &str, request: OpenRouterRequest) -> Result<OpenRouterResponse> {
    let client = reqwest::Client::new();
    
    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .context("failed to send request to openrouter api")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        
        if status == 400 && error_text.to_lowercase().contains("model") {
            return Err(anyhow::anyhow!(
                "invalid model '{}'. use the model settings menu to select a different model", 
                request.model
            ));
        } else {
            return Err(anyhow::anyhow!("openrouter api error ({}): {}", status, error_text));
        }
    }
    
    let response_body = response
        .json::<OpenRouterResponse>()
        .await
        .context("failed to parse openrouter api response")?;
    
    Ok(response_body)
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
        diff_info
    );
    
    // generate suggested bullet points if body needed
    if intelligence.requires_body {
        intelligence.suggested_bullets = generate_bullet_suggestions(&intelligence.detected_patterns);
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
            description: format!("{} new file{} introduced", 
                file_analysis.new_files.len(),
                if file_analysis.new_files.len() > 1 { "s" } else { "" }
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
            description: format!("changes span {} layers: {}", 
                layers.len(), 
                layers.join(", ")
            ),
            impact: 0.8 + (0.1 * layers.len() as f32),
            files_affected: diff_info.files.iter().map(|f| f.path.clone()).collect(),
        });
    }
    
    // mass modification pattern
    if diff_info.files.len() >= 5 {
        let total_changes: usize = diff_info.files.iter()
            .map(|f| f.added_lines + f.removed_lines)
            .sum();
        
        patterns.push(Pattern {
            pattern_type: PatternType::MassModification,
            description: format!("{} files modified with {} total line changes", 
                diff_info.files.len(), total_changes
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
            description: format!("{} test file{} modified", 
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
    
    patterns
}

fn analyse_file_metadata(diff_info: &DiffInfo) -> FileAnalysis {
    let mut analysis = FileAnalysis {
        new_files: Vec::new(),
        modified_files: Vec::new(),
        by_extension: HashMap::new(),
        by_directory: HashMap::new(),
    };
    
    for file in &diff_info.files {
        // new vs modified
        if file.removed_lines == 0 && file.added_lines > 5 {
            analysis.new_files.push(file.path.clone());
        } else {
            analysis.modified_files.push(file.path.clone());
        }
        
        // by extension
        if let Some(ext) = std::path::Path::new(&file.path).extension() {
            let ext_str = ext.to_string_lossy().to_string();
            analysis.by_extension.entry(ext_str).or_default().push(file.path.clone());
        }
        
        // by directory
        if let Some(parent) = std::path::Path::new(&file.path).parent() {
            let dir = parent.to_string_lossy().to_string();
            analysis.by_directory.entry(dir).or_default().push(file.path.clone());
        }
    }
    
    analysis
}

fn detect_layers(analysis: &FileAnalysis) -> Vec<String> {
    let mut layers = HashSet::new();
    
    // extension-based layer detection
    let frontend_exts = ["js", "jsx", "ts", "tsx", "vue", "svelte", "html", "css", "scss", "sass", "less"];
    let backend_exts = ["cs", "java", "py", "rb", "php", "go", "rs", "cpp", "c"];
    let mobile_exts = ["swift", "kt", "dart", "m", "mm"];
    let config_exts = ["json", "yaml", "yml", "toml", "ini", "env", "config", "conf"];
    let db_exts = ["sql", "migration", "schema"];
    let view_exts = ["cshtml", "razor", "erb", "ejs", "pug", "hbs"];
    
    for (ext, _) in &analysis.by_extension {
        let ext_lower = ext.to_lowercase();
        
        if frontend_exts.contains(&ext_lower.as_str()) {
            layers.insert("frontend");
        }
        if backend_exts.contains(&ext_lower.as_str()) {
            layers.insert("backend");
        }
        if mobile_exts.contains(&ext_lower.as_str()) {
            layers.insert("mobile");
        }
        if config_exts.contains(&ext_lower.as_str()) {
            layers.insert("configuration");
        }
        if db_exts.contains(&ext_lower.as_str()) {
            layers.insert("database");
        }
        if view_exts.contains(&ext_lower.as_str()) {
            layers.insert("views");
        }
    }
    
    // directory-based layer detection
    for (dir, _) in &analysis.by_directory {
        let dir_lower = dir.to_lowercase();
        
        if dir_lower.contains("frontend") || dir_lower.contains("client") || dir_lower.contains("ui") {
            layers.insert("frontend");
        }
        if dir_lower.contains("backend") || dir_lower.contains("server") || dir_lower.contains("api") {
            layers.insert("backend");
        }
        if dir_lower.contains("database") || dir_lower.contains("migrations") || dir_lower.contains("db") {
            layers.insert("database");
        }
        if dir_lower.contains("test") || dir_lower.contains("spec") {
            layers.insert("testing");
        }
        if dir_lower.contains("docs") || dir_lower.contains("documentation") {
            layers.insert("documentation");
        }
    }
    
    layers.into_iter().map(|s| s.to_string()).collect()
}

fn analyse_file_content_universal(file: &ModifiedFile) -> Vec<Pattern> {
    let mut patterns = Vec::new();
    
    // universal content patterns that work across languages
    let content_analysis = analyse_content(&file.diff_content);
    
    // feature detection
    if content_analysis.new_functions >= 3 || content_analysis.new_classes >= 1 {
        patterns.push(Pattern {
            pattern_type: PatternType::FeatureAddition,
            description: format!("new functionality added: {} functions, {} classes/types",
                content_analysis.new_functions, content_analysis.new_classes
            ),
            impact: 0.7,
            files_affected: vec![file.path.clone()],
        });
    }
    
    // interface changes
    if content_analysis.api_changes > 0 {
        patterns.push(Pattern {
            pattern_type: PatternType::InterfaceEvolution,
            description: "api or interface modifications detected".to_string(),
            impact: 0.8,
            files_affected: vec![file.path.clone()],
        });
    }
    
    // bug fix patterns
    if content_analysis.has_bug_fix_indicators {
        patterns.push(Pattern {
            pattern_type: PatternType::BugFixPattern,
            description: "bug fix or error handling improvements".to_string(),
            impact: 0.6,
            files_affected: vec![file.path.clone()],
        });
    }
    
    // performance patterns
    if content_analysis.has_performance_indicators {
        patterns.push(Pattern {
            pattern_type: PatternType::PerformanceTuning,
            description: "performance optimisations detected".to_string(),
            impact: 0.6,
            files_affected: vec![file.path.clone()],
        });
    }
    
    patterns
}

fn analyse_content(diff_content: &str) -> ContentAnalysis {
    let mut analysis = ContentAnalysis {
        new_functions: 0,
        new_classes: 0,
        api_changes: 0,
        has_bug_fix_indicators: false,
        has_performance_indicators: false,
    };
    
    let added_lines: Vec<&str> = diff_content.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| l.trim_start_matches('+').trim())
        .collect();
    
    // use more efficient string matching for common patterns instead of regex
    for line in &added_lines {
        let line_trimmed = line.trim();
        
        // count functions using simple string checks
        if line_trimmed.contains("fn ") || line_trimmed.contains("function ") || 
           line_trimmed.contains("def ") || line_trimmed.contains("func ") ||
           (line_trimmed.contains("(") && (line_trimmed.contains("public") || line_trimmed.contains("private"))) {
            analysis.new_functions += 1;
        }
        
        // count classes using simple string checks
        if line_trimmed.contains("class ") || line_trimmed.contains("struct ") || 
           line_trimmed.contains("interface ") || line_trimmed.contains("enum ") ||
           line_trimmed.contains("type ") {
            analysis.new_classes += 1;
        }
        
        // count api changes using simple string checks
        if line_trimmed.contains("@Get") || line_trimmed.contains("@Post") || 
           line_trimmed.contains("@Put") || line_trimmed.contains("@Delete") ||
           line_trimmed.contains("app.get") || line_trimmed.contains("router.") ||
           line_trimmed.contains("[Http") {
            analysis.api_changes += 1;
        }
    }
    
    // bug fix indicators
    let bug_keywords = ["fix", "bug", "error", "issue", "problem", "crash", "null", "undefined", "exception"];
    analysis.has_bug_fix_indicators = added_lines.iter().any(|line| {
        let line_lower = line.to_lowercase();
        bug_keywords.iter().any(|kw| line_lower.contains(kw))
    });
    
    // performance indicators
    let perf_keywords = ["cache", "optimize", "performance", "speed", "fast", "async", "parallel", "memo"];
    analysis.has_performance_indicators = added_lines.iter().any(|line| {
        let line_lower = line.to_lowercase();
        perf_keywords.iter().any(|kw| line_lower.contains(kw))
    });
    
    analysis
}

fn detect_config_files(analysis: &FileAnalysis) -> Vec<String> {
    let config_indicators = [
        "config", "settings", "env", "appsettings", "web.config", "app.config",
        ".json", ".yaml", ".yml", ".toml", ".ini", ".properties"
    ];
    
    analysis.new_files.iter().chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            config_indicators.iter().any(|ind| lower.contains(ind))
        })
        .cloned()
        .collect()
}

fn detect_dependency_files(analysis: &FileAnalysis) -> Vec<String> {
    let dep_files = [
        "package.json", "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
        "Cargo.toml", "Cargo.lock", "go.mod", "go.sum",
        "requirements.txt", "Pipfile", "poetry.lock",
        "*.csproj", "packages.config", "*.sln",
        "pom.xml", "build.gradle", "composer.json"
    ];
    
    analysis.new_files.iter().chain(&analysis.modified_files)
        .filter(|f| {
            dep_files.iter().any(|pattern| {
                if pattern.contains('*') {
                    f.ends_with(&pattern.replace("*", ""))
                } else {
                    f.ends_with(pattern)
                }
            })
        })
        .cloned()
        .collect()
}

fn detect_test_files(analysis: &FileAnalysis) -> Vec<String> {
    analysis.new_files.iter().chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("test") || lower.contains("spec") || 
            lower.ends_with(".test.js") || lower.ends_with(".spec.ts") ||
            lower.ends_with("tests.cs") || lower.ends_with("test.cs")
        })
        .cloned()
        .collect()
}

fn detect_refactoring_patterns(diff_info: &DiffInfo) -> RefactoringSignals {
    let high_churn_files: Vec<_> = diff_info.files.iter()
        .filter(|f| f.added_lines > 20 && f.removed_lines > 20)
        .collect();
    
    let total_added: usize = diff_info.files.iter().map(|f| f.added_lines).sum();
    let total_removed: usize = diff_info.files.iter().map(|f| f.removed_lines).sum();
    
    let is_refactoring = high_churn_files.len() >= 2 || 
        (total_added > 50 && total_removed > 50 && 
         (total_removed as f32 / total_added as f32) > 0.4);
    
    RefactoringSignals {
        is_refactoring,
        description: if high_churn_files.len() >= 2 {
            format!("{} files significantly restructured", high_churn_files.len())
        } else {
            "code reorganisation detected".to_string()
        },
        files: high_churn_files.iter().map(|f| f.path.clone()).collect(),
    }
}

fn calculate_new_file_impact(new_files: &[String]) -> f32 {
    match new_files.len() {
        0 => 0.0,
        1 => 0.5,
        2..=3 => 0.7,
        4..=5 => 0.8,
        _ => 0.9,
    }
}

fn calculate_pattern_complexity(patterns: &[Pattern]) -> f32 {
    let base_score: f32 = patterns.iter().map(|p| p.impact).sum();
    
    // bonus for pattern diversity
    let unique_types: HashSet<_> = patterns.iter().map(|p| &p.pattern_type).collect();
    let diversity_bonus = (unique_types.len() as f32 - 1.0) * 0.2;
    
    (base_score + diversity_bonus).min(5.0)
}

fn determine_body_requirement(patterns: &[Pattern], complexity: f32, _diff_info: &DiffInfo) -> bool {
    // high complexity always needs body
    if complexity >= 2.5 {
        return true;
    }
    
    // multiple high-impact patterns
    let high_impact_patterns = patterns.iter().filter(|p| p.impact >= 0.7).count();
    if high_impact_patterns >= 2 {
        return true;
    }
    
    // cross-layer changes always need explanation
    if patterns.iter().any(|p| matches!(p.pattern_type, PatternType::CrossLayerChange)) {
        return true;
    }
    
    // multiple features added
    let feature_count = patterns.iter()
        .filter(|p| matches!(p.pattern_type, PatternType::FeatureAddition | PatternType::NewFilePattern))
        .count();
    if feature_count >= 2 {
        return true;
    }
    
    // architectural or interface changes
    if patterns.iter().any(|p| matches!(p.pattern_type, 
        PatternType::ArchitecturalShift | PatternType::InterfaceEvolution
    )) {
        return true;
    }
    
    // mixed patterns
    let unique_types: HashSet<_> = patterns.iter().map(|p| &p.pattern_type).collect();
    if unique_types.len() >= 3 && complexity >= 1.5 {
        return true;
    }
    
    false
}

fn generate_bullet_suggestions(patterns: &[Pattern]) -> Vec<String> {
    let mut bullets = Vec::new();
    
    // group patterns by type for better organisation
    let mut grouped: HashMap<PatternType, Vec<&Pattern>> = HashMap::new();
    for pattern in patterns {
        grouped.entry(pattern.pattern_type.clone()).or_default().push(pattern);
    }
    
    // generate bullets based on pattern types
    for (pattern_type, group) in grouped {
        match pattern_type {
            PatternType::NewFilePattern => {
                let files = group.iter().flat_map(|p| &p.files_affected).collect::<Vec<_>>();
                bullets.push(format!("add {} new file{}: {}", 
                    files.len(),
                    if files.len() > 1 { "s" } else { "" },
                    summarize_files(&files)
                ));
            },
            PatternType::FeatureAddition => {
                for pattern in group {
                    bullets.push(pattern.description.clone());
                }
            },
            PatternType::CrossLayerChange => {
                bullets.push("implement changes across multiple application layers".to_string());
            },
            PatternType::RefactoringPattern => {
                bullets.push("refactor code for better maintainability".to_string());
            },
            PatternType::ConfigurationDrift => {
                bullets.push("update configuration settings".to_string());
            },
            PatternType::InterfaceEvolution => {
                bullets.push("modify api contracts or interfaces".to_string());
            },
            _ => {
                // generic bullet for other patterns
                if let Some(pattern) = group.first() {
                    bullets.push(pattern.description.clone());
                }
            }
        }
    }
    
    bullets
}

fn summarize_files(files: &[&String]) -> String {
    if files.len() <= 3 {
        files.iter().map(|f| {
            std::path::Path::new(f).file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(f)
        }).collect::<Vec<_>>().join(", ")
    } else {
        format!("{} and {} more", 
            files.iter().take(2).map(|f| {
                std::path::Path::new(f).file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(f)
            }).collect::<Vec<_>>().join(", "),
            files.len() - 2
        )
    }
}

fn suggest_commit_metadata(patterns: &[Pattern], diff_info: &DiffInfo) -> (String, Option<String>) {
    // determine commit type based on dominant pattern
    let mut type_scores: HashMap<&str, f32> = HashMap::new();
    
    for pattern in patterns {
        match pattern.pattern_type {
            PatternType::FeatureAddition | PatternType::NewFilePattern => {
                *type_scores.entry("feat").or_default() += pattern.impact;
            },
            PatternType::BugFixPattern => {
                *type_scores.entry("fix").or_default() += pattern.impact * 1.5; // prioritise fix
            },
            PatternType::RefactoringPattern => {
                *type_scores.entry("refactor").or_default() += pattern.impact;
            },
            PatternType::TestEvolution => {
                *type_scores.entry("test").or_default() += pattern.impact;
            },
            PatternType::DocumentationUpdate => {
                *type_scores.entry("docs").or_default() += pattern.impact;
            },
            PatternType::PerformanceTuning => {
                *type_scores.entry("perf").or_default() += pattern.impact;
            },
            PatternType::ConfigurationDrift | PatternType::DependencyUpdate => {
                *type_scores.entry("build").or_default() += pattern.impact * 0.8;
            },
            PatternType::StyleNormalization => {
                *type_scores.entry("style").or_default() += pattern.impact;
            },
            _ => {
                *type_scores.entry("feat").or_default() += pattern.impact * 0.5;
            }
        }
    }
    
    let commit_type = type_scores.iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(t, _)| t.to_string())
        .unwrap_or_else(|| "feat".to_string());
    
    // determine scope based on common directory or module
    let scope = determine_intelligent_scope(diff_info);
    
    (commit_type, scope)
}

fn determine_intelligent_scope(diff_info: &DiffInfo) -> Option<String> {
    // find the most common meaningful directory
    let mut dir_counts: HashMap<String, usize> = HashMap::new();
    
    for file in &diff_info.files {
        if let Some(parent) = std::path::Path::new(&file.path).parent() {
            // skip generic directories
            let skip_dirs = ["src", "lib", "app", "test", "tests", "spec"];
            
            for component in parent.components() {
                if let Some(comp_str) = component.as_os_str().to_str() {
                    if !skip_dirs.contains(&comp_str) && comp_str.len() > 2 {
                        *dir_counts.entry(comp_str.to_string()).or_default() += 1;
                    }
                }
            }
        }
    }
    
    // if one directory dominates, use it as scope
    if let Some((dir, count)) = dir_counts.iter().max_by_key(|(_k, v)| *v) {
        if *count as f32 / diff_info.files.len() as f32 > 0.5 {
            return Some(dir.to_lowercase());
        }
    }
    
    // otherwise, try to determine by file type
    let extensions: HashSet<_> = diff_info.files.iter()
        .filter_map(|f| std::path::Path::new(&f.path).extension())
        .map(|e| e.to_string_lossy().to_string())
        .collect();
    
    // map common extensions to scopes
    if extensions.iter().any(|e| ["ts", "js", "tsx", "jsx"].contains(&e.as_str())) {
        if extensions.iter().any(|e| ["css", "scss", "sass"].contains(&e.as_str())) {
            return Some("ui".to_string());
        }
        return Some("frontend".to_string());
    }
    
    if extensions.iter().any(|e| ["cs", "java", "py", "go"].contains(&e.as_str())) {
        return Some("api".to_string());
    }
    
    None
}

/// construct intelligent prompt using commit analysis
fn construct_intelligent_prompt(diff_info: &DiffInfo, intelligence: &CommitIntelligence) -> String {
    let mut prompt = String::new();
    
    // start with clear intent
    prompt.push_str("generate a conventional commit message based on the following analysis:\n\n");
    
    // complexity assessment
    prompt.push_str(&format!("📊 COMMIT COMPLEXITY: {:.1}/5.0 - {}\n",
        intelligence.complexity_score,
        if intelligence.complexity_score < 1.5 { "simple" }
        else if intelligence.complexity_score < 2.5 { "moderate" }
        else { "complex" }
    ));
    
    // body requirement (crystal clear)
    if intelligence.requires_body {
        prompt.push_str("\n🔴 BODY REQUIRED - this commit is too complex for a single line.\n");
        prompt.push_str("the commit message MUST include a body with bullet points.\n\n");
    } else {
        prompt.push_str("\n✅ SINGLE LINE - this is a focused change, no body needed.\n\n");
    }
    
    // detected patterns
    prompt.push_str("🔍 DETECTED PATTERNS:\n");
    for pattern in &intelligence.detected_patterns {
        prompt.push_str(&format!("- {}: {} (impact: {:.1})\n", 
            format_pattern_type(&pattern.pattern_type),
            pattern.description,
            pattern.impact
        ));
    }
    prompt.push_str("\n");
    
    // suggested structure
    prompt.push_str("📝 SUGGESTED COMMIT STRUCTURE:\n");
    prompt.push_str(&format!("type: {}\n", intelligence.commit_type_hint));
    if let Some(scope) = &intelligence.scope_hint {
        prompt.push_str(&format!("scope: {}\n", scope));
    }
    prompt.push_str("\n");
    
    // if body required, provide bullet suggestions
    if intelligence.requires_body && !intelligence.suggested_bullets.is_empty() {
        prompt.push_str("📌 SUGGESTED BULLET POINTS FOR BODY:\n");
        for bullet in &intelligence.suggested_bullets {
            prompt.push_str(&format!("- {}\n", bullet));
        }
        prompt.push_str("\n");
    }
    
    // actual diff summary with emphasis on code changes
    prompt.push_str("📁 DETAILED CHANGE ANALYSIS:\n");
    prompt.push_str(&diff_info.summary);
    prompt.push_str("\n");
    
    // add specific file-level insights (limited for performance)
    if !diff_info.files.is_empty() {
        prompt.push_str("\n🔍 SPECIFIC CHANGES PER FILE:\n");
        for (i, file) in diff_info.files.iter().enumerate() {
            if i >= 3 { break; } // limit to 3 files for speed and brevity
            prompt.push_str(&format!("{}:\n", file.path));
            
            // analyse specific changes in this file (only for smaller files)
            if file.diff_content.len() < 10000 { // skip analysis for very large files
                let file_changes = analyse_file_changes_for_prompt(&file.diff_content);
                if !file_changes.is_empty() {
                    prompt.push_str(&format!("  - {}\n", file_changes.join("\n  - ")));
                } else {
                    prompt.push_str(&format!("  - {} lines added, {} removed\n", file.added_lines, file.removed_lines));
                }
            } else {
                prompt.push_str(&format!("  - {} lines added, {} removed (large file)\n", file.added_lines, file.removed_lines));
            }
        }
        
        if diff_info.files.len() > 3 {
            prompt.push_str(&format!("... and {} more files\n", diff_info.files.len() - 3));
        }
    }
    prompt.push_str("\n");
    
    // provide specific examples based on detected patterns
    prompt.push_str("✨ EXAMPLES FOR THIS TYPE OF CHANGE:\n");
    if intelligence.requires_body {
        // provide examples that match the detected patterns
        let has_new_files = intelligence.detected_patterns.iter()
            .any(|p| matches!(p.pattern_type, PatternType::NewFilePattern));
        let has_refactoring = intelligence.detected_patterns.iter()
            .any(|p| matches!(p.pattern_type, PatternType::RefactoringPattern));
        let has_features = intelligence.detected_patterns.iter()
            .any(|p| matches!(p.pattern_type, PatternType::FeatureAddition));
        
        if has_new_files && has_features {
            prompt.push_str("```\n");
            prompt.push_str("feat(core): implement intelligent commit message generation\n\n");
            prompt.push_str("- add generate_conventional_commit and analyse_commit_intelligence functions\n");
            prompt.push_str("- implement PatternType enum with 15 distinct change patterns\n");
            prompt.push_str("- add ai.rs module with pattern detection algorithms\n");
            prompt.push_str("- integrate regex dependency for content analysis\n");
            prompt.push_str("- enhance diff parsing to extract meaningful code changes\n");
            prompt.push_str("\nBREAKING CHANGE: api interface changed requiring updated imports\n");
            prompt.push_str("```\n\n");
        } else if has_refactoring {
            prompt.push_str("```\n");
            prompt.push_str("refactor(parser): restructure diff analysis for better accuracy\n\n");
            prompt.push_str("- extract pattern detection into separate functions\n");
            prompt.push_str("- improve code organisation and readability\n");
            prompt.push_str("- consolidate duplicate analysis logic\n");
            prompt.push_str("```\n\n");
        } else {
            prompt.push_str("```\n");
            prompt.push_str(&format!("{}({}): {}\n\n", 
                intelligence.commit_type_hint,
                intelligence.scope_hint.as_deref().unwrap_or("scope"),
                "describe the main change briefly"
            ));
            prompt.push_str("- explain first major change\n");
            prompt.push_str("- describe second significant modification\n");
            prompt.push_str("- note any important technical details\n");
            prompt.push_str("```\n\n");
        }
    } else {
        // simple single-line examples
        prompt.push_str("```\n");
        match intelligence.commit_type_hint.as_str() {
            "feat" => {
                prompt.push_str("feat(parser): add support for rust function detection\n");
                prompt.push_str("feat!: implement breaking api changes\n");
                prompt.push_str("feat(cli): implement interactive commit message editing\n");
            },
            "fix" => {
                prompt.push_str("fix(git): resolve diff parsing for binary files\n");
                prompt.push_str("fix(ai): handle empty responses from openrouter api\n");
            },
            "refactor" => {
                prompt.push_str("refactor(core): extract pattern detection into modules\n");
                prompt.push_str("refactor(utils): simplify file type classification logic\n");
            },
            _ => {
                prompt.push_str(&format!("{}({}): {}\n", 
                    intelligence.commit_type_hint,
                    intelligence.scope_hint.as_deref().unwrap_or("scope"),
                    "brief description of change"
                ));
            }
        }
        prompt.push_str("```\n\n");
    }
    
    // clear instructions
    prompt.push_str("🎯 INSTRUCTIONS:\n");
    if intelligence.requires_body {
        prompt.push_str("1. create a commit with type, optional scope, and description (under 72 chars)\n");
        prompt.push_str("2. add a blank line\n");
        prompt.push_str("3. add a body with bullet points explaining the key changes\n");
        prompt.push_str("4. USE SPECIFIC NAMES: mention actual function names, types, and files from the analysis\n");
        prompt.push_str("5. ORGANISE BULLETS: major changes first, then features, then minor updates\n");
        prompt.push_str("6. BE SPECIFIC: avoid generic terms like 'update dependencies' - say what was updated\n");
        prompt.push_str("7. FOLLOW CONVENTIONAL COMMITS 1.0: use ! for breaking changes or BREAKING CHANGE: footer\n");
        prompt.push_str("8. FOOTERS: use BREAKING CHANGE: (all caps) for breaking changes in footer\n");
        prompt.push_str("9. focus on WHAT changed and WHY, not implementation details\n");
        prompt.push_str("10. use UK english spelling (optimisation, behaviour, etc.)\n");
    } else {
        prompt.push_str("1. create a single-line commit message\n");
        prompt.push_str("2. format: <type>(<scope>): <description>\n");
        prompt.push_str("3. description must be under 72 characters\n");
        prompt.push_str("4. NO BODY - just the single line\n");
        prompt.push_str("5. use UK english spelling\n");
    }
    
    prompt.push_str("\ngenerate the commit message now:\n");
    
    prompt
}

/// analyse file changes for detailed prompt context
fn analyse_file_changes_for_prompt(diff_content: &str) -> Vec<String> {
    let mut changes = Vec::new();
    let added_lines: Vec<&str> = diff_content.lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| l.trim_start_matches('+').trim())
        .filter(|l| !l.is_empty() && l.len() > 5) // filter out trivial changes
        .collect();
    
    let removed_lines: Vec<&str> = diff_content.lines()
        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
        .map(|l| l.trim_start_matches('-').trim())
        .filter(|l| !l.is_empty() && l.len() > 5)
        .collect();
    
    // detailed function/method analysis using cached regex
    let function_regex = get_function_regex();
    for line in &added_lines {
        if let Some(captures) = function_regex.captures(line) {
            if let Some(kind) = captures.get(3) {
                if let Some(name) = captures.get(4) {
                    let async_marker = if captures.get(2).is_some() { "async " } else { "" };
                    changes.push(format!("add {}{}function {}", async_marker, kind.as_str(), name.as_str()));
                }
            }
        }
    }
    
    // detect api/route changes
    let api_patterns = [
        r#"@(Get|Post|Put|Delete|Patch)\("([^"]+)""#,
        r#"(app|router)\.(get|post|put|delete|patch)\s*\(\s*['"]([^'"]+)['"]"#,
        r#"\[Http(Get|Post|Put|Delete)\]"#,
    ];
    
    for line in &added_lines {
        for pattern in &api_patterns {
            if let Ok(regex) = regex::Regex::new(pattern) {
                if let Some(captures) = regex.captures(line) {
                    if captures.len() >= 3 {
                        if let Some(route) = captures.get(captures.len() - 1) {
                            changes.push(format!("add api endpoint {}", route.as_str()));
                        }
                    } else {
                        changes.push("add api endpoint".to_string());
                    }
                    break;
                }
            }
        }
    }
    
    // detect import/dependency changes using cached regex
    let import_regex = get_import_regex();
    let mut import_count = 0;
    for line in &added_lines {
        if import_regex.is_match(line) {
            import_count += 1;
        }
    }
    if import_count > 0 {
        changes.push(format!("add {} new dependencies", import_count));
    }
    
    // detect configuration changes
    let config_indicators = ["config", "setting", "env", "const", "static"];
    for line in &added_lines {
        let line_lower = line.to_lowercase();
        if config_indicators.iter().any(|&indicator| line_lower.contains(indicator)) {
            changes.push("modify configuration values".to_string());
            break;
        }
    }
    
    // detect test additions
    if added_lines.iter().any(|line| {
        let line_lower = line.to_lowercase();
        line_lower.contains("test") || line_lower.contains("spec") || 
        line_lower.contains("assert") || line_lower.contains("expect")
    }) {
        changes.push("add tests".to_string());
    }
    
    // detect error handling improvements
    let error_patterns = ["Result", "Error", "Exception", "try", "catch", "unwrap", "expect"];
    for line in &added_lines {
        if error_patterns.iter().any(|&pattern| line.contains(pattern)) {
            changes.push("improve error handling".to_string());
            break;
        }
    }
    
    // detect performance/async additions
    let perf_patterns = ["async", "await", "cache", "optimize", "performance", "parallel"];
    for line in &added_lines {
        let line_lower = line.to_lowercase();
        if perf_patterns.iter().any(|&pattern| line_lower.contains(pattern)) {
            changes.push("add performance optimisations".to_string());
            break;
        }
    }
    
    // analyse major refactoring (high add/remove ratio)
    if removed_lines.len() > 10 && added_lines.len() > 10 {
        let ratio = removed_lines.len() as f32 / added_lines.len() as f32;
        if ratio > 0.7 && ratio < 1.3 {
            changes.push("refactor existing implementation".to_string());
        }
    }
    
    // if no specific changes detected, provide general context
    if changes.is_empty() && !added_lines.is_empty() {
        if added_lines.len() > 20 {
            changes.push("major code additions".to_string());
        } else if added_lines.len() > 5 {
            changes.push("code modifications".to_string());
        }
    }
    
    // deduplicate and limit
    let mut unique_changes: Vec<String> = changes.into_iter().collect::<std::collections::HashSet<_>>().into_iter().collect();
    unique_changes.sort();
    unique_changes.truncate(4); // limit to top 4 changes per file
    
    unique_changes
}

fn format_pattern_type(pattern_type: &PatternType) -> &'static str {
    match pattern_type {
        PatternType::NewFilePattern => "new files",
        PatternType::MassModification => "mass changes",
        PatternType::CrossLayerChange => "cross-layer",
        PatternType::InterfaceEvolution => "api change",
        PatternType::ArchitecturalShift => "architecture",
        PatternType::ConfigurationDrift => "config",
        PatternType::DependencyUpdate => "dependencies",
        PatternType::RefactoringPattern => "refactoring",
        PatternType::FeatureAddition => "new feature",
        PatternType::BugFixPattern => "bug fix",
        PatternType::TestEvolution => "tests",
        PatternType::DocumentationUpdate => "docs",
        PatternType::StyleNormalization => "style",
        PatternType::PerformanceTuning => "performance",
        PatternType::SecurityHardening => "security",
    }
}

/// get appropriate system prompt based on intelligence
fn get_system_prompt(intelligence: &CommitIntelligence) -> &'static str {
    if intelligence.requires_body {
        SYSTEM_PROMPT_COMPLEX
    } else {
        SYSTEM_PROMPT
    }
}

/// select model based on complexity
pub fn select_model_for_complexity(intelligence: &CommitIntelligence, debug: bool, config: &Config) -> String {
    let (model, reason) = if intelligence.complexity_score < 1.5 {
        (
            &config.models.fast,
            "simple commit detected - using fast model"
        )
    } else if intelligence.complexity_score < 2.5 {
        (
            &config.models.thinking,
            "medium complexity commit - using thinking model"
        )
    } else {
        (
            &config.models.thinking,
            "complex commit detected - using thinking model"
        )
    };
    
    if debug {
        println!("🤖 smart model selection: {} ({})", reason, model);
    }
    
    model.to_string()
}

/// get available models for manual selection
pub fn get_available_models(config: &Config) -> Vec<(&str, &str)> {
    config.models.available.iter()
        .map(|model| (model.name.as_str(), model.description.as_str()))
        .collect()
}

/// normalize commit message format, converting [scope] to (scope)
fn normalize_commit_format(msg: &str) -> String {
    // convert type[scope]: description to type(scope): description
    if msg.contains('[') && msg.contains(']') && msg.contains(':') {
        let parts: Vec<&str> = msg.splitn(2, ':').collect();
        if parts.len() == 2 {
            let type_scope = parts[0].trim();
            let description = parts[1].trim();
            
            // replace [scope] with (scope)
            let normalized_type_scope = type_scope.replace('[', "(").replace(']', ")");
            return format!("{}: {}", normalized_type_scope, description);
        }
    }
    
    msg.to_string()
}

/// extract commit message from ai response
fn extract_commit_message(response: &str) -> String {
    // look for commit message in code blocks
    let lines: Vec<&str> = response.lines().collect();
    let mut in_code_block = false;
    let mut commit_lines = Vec::new();
    let mut found_commit_start = false;
    
    for line in lines {
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
                if trimmed.starts_with("Breaking changes:") || 
                   trimmed.starts_with("Note:") {
                    break;
                }
                
                // BREAKING CHANGE: footer is valid conventional commit content
                // Don't break on it, include it
                
                // we're in a commit message, collect all lines
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
    
    // fallback: look for single line commit messages
    for line in response.lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let cleaned = clean_commit_message(trimmed);
            if is_likely_commit_message(&cleaned) {
                return normalize_commit_format(&cleaned);
            }
        }
    }
    
    // last resort: return the first non-empty line
    let fallback = response.lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| clean_commit_message(line.trim()))
        .unwrap_or_else(|| "feat: update".to_string());
    
    normalize_commit_format(&fallback)
}

/// clean up commit message by removing markdown formatting
fn clean_commit_message(msg: &str) -> String {
    let lines: Vec<&str> = msg.lines().collect();
    let mut cleaned_lines = Vec::new();
    
    for line in lines {
        let mut cleaned = line.trim().to_string();
        
        // remove leading asterisks from bullet points but preserve the dash
        if cleaned.starts_with('*') && !cleaned.starts_with("* ") {
            cleaned = cleaned.trim_start_matches('*').trim().to_string();
        }
        
        // remove trailing asterisks
        while cleaned.ends_with('*') {
            cleaned = cleaned.trim_end_matches('*').to_string();
        }
        
        // remove backticks
        cleaned = cleaned.replace('`', "");
        
        // clean up double spaces
        while cleaned.contains("  ") {
            cleaned = cleaned.replace("  ", " ");
        }
        
        // remove quotes if present on single lines
        if cleaned.starts_with('"') && cleaned.ends_with('"') && !cleaned.contains('\n') {
            cleaned = cleaned.trim_matches('"').to_string();
        }
        
        cleaned_lines.push(cleaned.trim().to_string());
    }
    
    // join lines back together, preserving structure
    cleaned_lines.join("\n").trim().to_string()
}

/// check if a line looks like a conventional commit message
fn is_likely_commit_message(line: &str) -> bool {
    let valid_types = ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"];
    
    if line.contains(':') {
        let before_colon = line.split(':').next().unwrap_or("").trim();
        
        // handle type(scope) or type[scope] pattern
        let type_part = if before_colon.contains('(') {
            before_colon.split('(').next().unwrap_or("").trim()
        } else if before_colon.contains('[') {
            before_colon.split('[').next().unwrap_or("").trim()
        } else {
            before_colon
        };
        
        return valid_types.contains(&type_part);
    }
    
    false
}

/// validate that the generated commit message follows conventional commits format
fn validate_commit_message(msg: &str) -> Result<()> {
    let lines: Vec<&str> = msg.lines().collect();
    if lines.is_empty() {
        return Err(anyhow::anyhow!("commit message is empty"));
    }
    
    let first_line = lines[0];
    
    // check for valid conventional commit format
    let valid_types = ["feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert"];
    
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
        if !scope.is_empty() {
            if scope.contains(' ') || !scope.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                return Err(anyhow::anyhow!("invalid scope '{}', must be a noun (alphanumeric, hyphens, or underscores only)", scope));
            }
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
    
    // check for imperative mood
    let words: Vec<&str> = description.split_whitespace().collect();
    if let Some(first_word) = words.first() {
        if first_word.ends_with("ed") || (first_word.ends_with("ing") && first_word.len() > 4) {
            let non_imperative = ["added", "removed", "deleted", "created", "updated", "modified", 
                                "fixing", "adding", "removing", "creating", "updating", "modifying"];
            if non_imperative.contains(first_word) {
                return Err(anyhow::anyhow!("description should use imperative mood (e.g., 'add' not 'added' or 'adding')"));
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
    let filtered: Vec<&str> = words.into_iter()
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

/// post-process commit message to ensure it meets all requirements
fn post_process_commit_message(msg: &str) -> String {
    if let Some(colon_pos) = msg.find(':') {
        let type_scope = msg[..colon_pos].trim_end();
        let mut description = msg[colon_pos + 1..].trim().to_string();

        if description.is_empty() {
            return format!("{}: ", type_scope);
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
        
        return format!("{}: {}", type_scope, description);
    }
    
    msg.to_string()
}

// system prompts
const SYSTEM_PROMPT_COMPLEX: &str = r#"you are commitwizard, an expert at creating conventional commit messages for git commits.

CRITICAL: when BODY REQUIREMENT is HIGH or when multiple significant features are added, you MUST include a detailed commit body.

a good commit message for complex changes looks like:
```
type(scope): concise description under 72 chars

- first major change or feature added
- second major change with brief explanation
- configuration or infrastructure changes
- any breaking changes or important notes
```

the body should:
- use bullet points starting with "- "
- focus on WHAT changed, not HOW
- group related changes together
- mention configuration changes, new features, and api changes
- be informative but concise

for simple changes (under 50 lines, single feature), a one-line message is fine.
for complex changes (multiple features, configuration, cross-module), ALWAYS include a body."#;

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
- scope must be a noun describing the section of codebase being changed
- use contextual scopes based on actual changes: parser, auth, logger, api, etc.
- use NO scope if changes affect multiple unrelated components  
- scope should be specific and meaningful: "parser" not "code", "auth" not "security stuff"
- avoid generic scopes like "app", "project", "system", "general"

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

STRICT FOOTER RULES (Conventional Commits 1.0 spec):
- breaking changes MUST use "BREAKING CHANGE: description" (all caps)
- breaking changes can also use ! before colon: "feat!: description"
- footers use token: value or token # value format
- use - instead of spaces in footer tokens (except BREAKING CHANGE)
- no ticket references unless explicitly in diff

CRITICAL REQUIREMENTS:
1. follow the context analysis suggestions for type
2. generate contextual scopes based on what code sections are actually changed
3. be precise - "enhance prompt parsing" not "improve things"
4. prioritise suggested type over default assumptions  
5. no markdown formatting, backticks, or special characters
6. UK english spelling only (optimisation not optimization, behaviour not behavior, organisation not organization, colour not color, etc.)

output ONLY the commit message, no explanations or additional text."#;