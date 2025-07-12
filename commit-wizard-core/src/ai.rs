use crate::git::{DiffInfo, ModifiedFile};
use crate::Config;
use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::time::Duration;
use tokio::time::sleep;

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
    CiChange, // new: for ci configuration changes
    Deprecation, // new: for deprecation-related changes
    SecurityFix, // new: for security vulnerability fixes
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
pub async fn generate_conventional_commit(
    diff_info: &DiffInfo,
    debug: bool,
    smart_model: bool,
    config: &Config,
) -> Result<String> {
    generate_conventional_commit_with_model(diff_info, debug, smart_model, None, config).await
}

/// generate a conventional commit message with optional custom model
pub async fn generate_conventional_commit_with_model(
    diff_info: &DiffInfo,
    debug: bool,
    smart_model: bool,
    custom_model: Option<String>,
    config: &Config,
) -> Result<String> {
    // start spinner immediately to show activity
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("{spinner:.blue} {msg}")
            .unwrap(),
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
        println!("🐛 debug: Commit intelligence analysis:");
        println!(
            "  └─ Complexity score: {:.1}/5.0",
            intelligence.complexity_score
        );
        println!("  └─ Requires body: {}", intelligence.requires_body);
        println!(
            "  └─ Detected patterns: {}",
            intelligence.detected_patterns.len()
        );
        for pattern in &intelligence.detected_patterns {
            println!(
                "     • {}: {} (impact: {:.1})",
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

        println!("🐛 debug: File analysis summary:");
        for (i, file) in diff_info.files.iter().enumerate() {
            if i >= 3 {
                println!("  ... and {} more files", diff_info.files.len() - i);
                break;
            }
            println!(
                "  └─ {}: +{} -{} lines",
                file.path, file.added_lines, file.removed_lines
            );

            // show what diff content is being sent to AI
            let lines_to_include = if file.added_lines + file.removed_lines > 100 {
                15
            } else {
                20
            };
            let meaningful_diff =
                extract_meaningful_diff_lines(&file.diff_content, lines_to_include);
            if !meaningful_diff.is_empty() {
                println!(
                    "     📝 Diff sent to AI ({} lines):",
                    meaningful_diff.lines().count()
                );
                for (j, line) in meaningful_diff.lines().enumerate() {
                    match j.cmp(&3) {
                        std::cmp::Ordering::Less => {
                            // show first 3 lines as sample
                            println!("       {}", line);
                        }
                        std::cmp::Ordering::Equal => {
                            println!(
                                "       ... ({} more lines sent to AI)",
                                meaningful_diff.lines().count() - 3
                            );
                            break;
                        }
                        std::cmp::Ordering::Greater => break,
                    }
                }
            }
        }
        println!();

        println!("🐛 debug: Full prompt being sent to AI:");
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
            format!("{}\n\nIMPORTANT: The description MUST be under 72 characters. Be concise!\nMUST USE TYPE: {}\nMUST USE SCOPE: {}", 
                prompt,
                intelligence.commit_type_hint,
                intelligence.scope_hint.as_ref().unwrap_or(&"none".to_string())
            )
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

        let raw_response = match response.choices.first() {
            Some(choice) => choice.message.content.clone(),
            None => {
                eprintln!("Warning: Empty choices in response");
                String::new()
            }
        };

        if debug {
            println!("🐛 debug: Raw API response:");
            println!("═══════════════════════════════════════");
            println!("{}", raw_response);
            println!("═══════════════════════════════════════");
            println!();
        }

        let commit_msg = extract_commit_message(&raw_response);
        let commit_msg = post_process_commit_message(&commit_msg);

        if debug {
            println!("🐛 debug: Extracted and processed commit message:");
            println!("═══════════════════════════════════════");
            println!("'{}", commit_msg);
            println!("═══════════════════════════════════════");
            println!();
        }

        // Check if type matches hint
        let generated_type = commit_msg
            .split(':')
            .next()
            .unwrap_or("")
            .split('(')
            .next()
            .unwrap_or("")
            .split('!')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let expected_type = intelligence.commit_type_hint.clone();

        match validate_commit_message(&commit_msg) {
            Ok(()) => {
                if generated_type == expected_type {
                    break Ok(commit_msg);
                } else if retry_count < max_retries {
                    retry_count += 1;
                    if debug {
                        println!("⚠️  Generated type '{}' doesn't match expected '{}', retrying ({}/{})\n", generated_type, expected_type, retry_count, max_retries);
                    }
                    continue;
                } else {
                    break Ok(commit_msg); // Accept even if type doesn't match after retries
                }
            }
            Err(e) => {
                if e.to_string().contains("description too long") && retry_count < max_retries {
                    retry_count += 1;
                    if debug {
                        println!(
                            "⚠️  description too long, retrying ({}/{})\n",
                            retry_count, max_retries
                        );
                    }
                    continue;
                } else {
                    break Err(e);
                }
            }
        }
    };

    spinner.finish_and_clear();
    result
}

/// make api request to openrouter
async fn make_api_request(api_key: &str, request: OpenRouterRequest) -> Result<OpenRouterResponse> {
    let max_retries = 3;
    let mut retry_delay = Duration::from_secs(1);

    for attempt in 0..max_retries {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        let response = client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await;

        match response {
            Ok(resp) => {
                if resp.status().is_success() {
                    let body = resp
                        .json::<OpenRouterResponse>()
                        .await
                        .context("failed to parse openrouter api response")?;
                    return Ok(body);
                } else {
                    let status = resp.status();
                    let error_text = resp
                        .text()
                        .await
                        .unwrap_or_else(|_| "unknown error".to_string());

                    if status.is_server_error() || status == 429 {
                        if attempt < max_retries - 1 {
                            eprintln!(
                                "Retryable error ({}): {}. Retrying in {:?}...",
                                status, error_text, retry_delay
                            );
                            sleep(retry_delay).await;
                            retry_delay *= 2;
                            continue;
                        }
                    } else if status == 400 && error_text.to_lowercase().contains("model") {
                        return Err(anyhow::anyhow!(
                            "invalid model '{}'. use the model settings menu to select a different model",
                            request.model
                        ));
                    }

                    return Err(anyhow::anyhow!(
                        "openrouter api error ({}): {}",
                        status,
                        error_text
                    ));
                }
            }
            Err(e) => {
                if attempt < max_retries - 1 {
                    eprintln!("Network error: {}. Retrying in {:?}...", e, retry_delay);
                    sleep(retry_delay).await;
                    retry_delay *= 2;
                    continue;
                } else {
                    return Err(anyhow::anyhow!(
                        "failed to send request to openrouter api: {}",
                        e
                    ));
                }
            }
        }
    }

    Err(anyhow::anyhow!("max retries exceeded for api request"))
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

    // new: documentation changes
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

    // new: style changes
    if detect_style_changes(diff_info) {
        patterns.push(Pattern {
            pattern_type: PatternType::StyleNormalization,
            description: "formatting and style normalisations".to_string(),
            impact: 0.3,
            files_affected: diff_info.files.iter().map(|f| f.path.clone()).collect(),
        });
    }

    // new: ci changes
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

    // new: deprecation patterns
    let deprecation_files: Vec<String> = diff_info
        .files
        .iter()
        .filter(|file| {
            let content_lower = file.diff_content.to_lowercase();
            // detect @deprecated annotations or deprecation comments
            content_lower.contains("@deprecated") ||
        content_lower.contains("deprecated") ||
        // detect removed exports (heuristic: significant removals with export keywords)
        (file.removed_lines > 10 && content_lower.contains("export"))
        })
        .map(|f| f.path.clone())
        .collect();

    if !deprecation_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::Deprecation,
            description: format!(
                "{} deprecation{} detected - potential breaking changes",
                deprecation_files.len(),
                if deprecation_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.9, // high impact as deprecations often indicate breaking changes
            files_affected: deprecation_files,
        });
    }

    // new: security fix patterns
    let security_files: Vec<String> = diff_info
        .files
        .iter()
        .filter(|file| {
            let content_lower = file.diff_content.to_lowercase();
            let path_lower = file.path.to_lowercase();
            
            // detect security-related keywords in diff content
            let has_security_keywords = content_lower.contains("vulnerability") ||
                content_lower.contains("security") ||
                content_lower.contains("exploit") ||
                content_lower.contains("injection") ||
                content_lower.contains("xss") ||
                content_lower.contains("csrf") ||
                content_lower.contains("authentication") ||
                content_lower.contains("authorization") ||
                content_lower.contains("sanitiz") ||
                content_lower.contains("escape") ||
                content_lower.contains("validate") ||
                content_lower.contains("permission") ||
                content_lower.contains("encrypt") ||
                content_lower.contains("hash") ||
                content_lower.contains("secret") ||
                content_lower.contains("token") ||
                content_lower.contains("credential") ||
                content_lower.contains("login") ||
                content_lower.contains("password");
            
            // detect security-related file paths
            let has_security_paths = path_lower.contains("auth") ||
                path_lower.contains("security") ||
                path_lower.contains("permission") ||
                path_lower.contains("login") ||
                path_lower.contains("middleware") ||
                path_lower.contains("guard");
                
            // detect security fixes in commit patterns (added validations, fixes, etc.)
            let has_security_fixes = (content_lower.contains("fix") || content_lower.contains("patch")) &&
                (content_lower.contains("auth") || content_lower.contains("security") || 
                 content_lower.contains("validate") || content_lower.contains("sanitiz"));
            
            has_security_keywords || has_security_paths || has_security_fixes
        })
        .map(|f| f.path.clone())
        .collect();

    if !security_files.is_empty() {
        patterns.push(Pattern {
            pattern_type: PatternType::SecurityFix,
            description: format!(
                "{} security-related change{} detected",
                security_files.len(),
                if security_files.len() > 1 { "s" } else { "" }
            ),
            impact: 0.95, // very high impact as security issues are critical
            files_affected: security_files,
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
            analysis
                .by_extension
                .entry(ext_str)
                .or_default()
                .push(file.path.clone());
        }

        // by directory
        if let Some(parent) = std::path::Path::new(&file.path).parent() {
            let dir = parent.to_string_lossy().to_string();
            analysis
                .by_directory
                .entry(dir)
                .or_default()
                .push(file.path.clone());
        }
    }

    analysis
}

fn detect_layers(analysis: &FileAnalysis) -> Vec<String> {
    let mut layers = HashSet::new();

    // extension-based layer detection
    let frontend_exts = [
        "js", "jsx", "ts", "tsx", "vue", "svelte", "html", "css", "scss", "sass", "less",
    ];
    let backend_exts = ["cs", "java", "py", "rb", "php", "go", "rs", "cpp", "c"];
    let mobile_exts = ["swift", "kt", "dart", "m", "mm"];
    let config_exts = [
        "json", "yaml", "yml", "toml", "ini", "env", "config", "conf",
    ];
    let db_exts = ["sql", "migration", "schema"];
    let view_exts = ["cshtml", "razor", "erb", "ejs", "pug", "hbs"];

    for ext in analysis.by_extension.keys() {
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
    for dir in analysis.by_directory.keys() {
        let dir_lower = dir.to_lowercase();

        if dir_lower.contains("frontend")
            || dir_lower.contains("client")
            || dir_lower.contains("ui")
        {
            layers.insert("frontend");
        }
        if dir_lower.contains("backend")
            || dir_lower.contains("server")
            || dir_lower.contains("api")
        {
            layers.insert("backend");
        }
        if dir_lower.contains("database")
            || dir_lower.contains("migrations")
            || dir_lower.contains("db")
        {
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
            description: format!(
                "new functionality added: {} functions, {} classes/types",
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

    let added_lines: Vec<&str> = diff_content
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| l.trim_start_matches('+').trim())
        .collect();

    // use more efficient string matching for common patterns instead of regex
    for line in &added_lines {
        let line_trimmed = line.trim();

        // count functions using simple string checks
        if line_trimmed.contains("fn ")
            || line_trimmed.contains("function ")
            || line_trimmed.contains("def ")
            || line_trimmed.contains("func ")
            || (line_trimmed.contains("(")
                && (line_trimmed.contains("public") || line_trimmed.contains("private")))
        {
            analysis.new_functions += 1;
        }

        // count classes using simple string checks
        if line_trimmed.contains("class ")
            || line_trimmed.contains("struct ")
            || line_trimmed.contains("interface ")
            || line_trimmed.contains("enum ")
            || line_trimmed.contains("type ")
        {
            analysis.new_classes += 1;
        }

        // count api changes using simple string checks
        if line_trimmed.contains("@Get")
            || line_trimmed.contains("@Post")
            || line_trimmed.contains("@Put")
            || line_trimmed.contains("@Delete")
            || line_trimmed.contains("app.get")
            || line_trimmed.contains("router.")
            || line_trimmed.contains("[Http")
        {
            analysis.api_changes += 1;
        }
    }

    // bug fix indicators
    let bug_keywords = [
        "fix",
        "bug",
        "error",
        "issue",
        "problem",
        "crash",
        "null",
        "undefined",
        "exception",
    ];
    analysis.has_bug_fix_indicators = added_lines.iter().any(|line| {
        let line_lower = line.to_lowercase();
        bug_keywords.iter().any(|kw| line_lower.contains(kw))
    });

    // performance indicators
    let perf_keywords = [
        "cache",
        "optimize",
        "performance",
        "speed",
        "fast",
        "async",
        "parallel",
        "memo",
    ];
    analysis.has_performance_indicators = added_lines.iter().any(|line| {
        let line_lower = line.to_lowercase();
        perf_keywords.iter().any(|kw| line_lower.contains(kw))
    });

    analysis
}

fn detect_config_files(analysis: &FileAnalysis) -> Vec<String> {
    let config_indicators = [
        "config",
        "settings",
        "env",
        "appsettings",
        "web.config",
        "app.config",
        ".json",
        ".yaml",
        ".yml",
        ".toml",
        ".ini",
        ".properties",
    ];

    analysis
        .new_files
        .iter()
        .chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            config_indicators.iter().any(|ind| lower.contains(ind))
        })
        .cloned()
        .collect()
}

fn detect_dependency_files(analysis: &FileAnalysis) -> Vec<String> {
    let dep_files = [
        "package.json",
        "package-lock.json",
        "yarn.lock",
        "pnpm-lock.yaml",
        "Cargo.toml",
        "Cargo.lock",
        "go.mod",
        "go.sum",
        "requirements.txt",
        "Pipfile",
        "poetry.lock",
        "*.csproj",
        "packages.config",
        "*.sln",
        "pom.xml",
        "build.gradle",
        "composer.json",
    ];

    analysis
        .new_files
        .iter()
        .chain(&analysis.modified_files)
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
    analysis
        .new_files
        .iter()
        .chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            lower.contains("test")
                || lower.contains("spec")
                || lower.ends_with(".test.js")
                || lower.ends_with(".spec.ts")
                || lower.ends_with("tests.cs")
                || lower.ends_with("test.cs")
        })
        .cloned()
        .collect()
}

fn detect_refactoring_patterns(diff_info: &DiffInfo) -> RefactoringSignals {
    let high_churn_files: Vec<_> = diff_info
        .files
        .iter()
        .filter(|f| f.added_lines > 20 && f.removed_lines > 20)
        .collect();

    let total_added: usize = diff_info.files.iter().map(|f| f.added_lines).sum();
    let total_removed: usize = diff_info.files.iter().map(|f| f.removed_lines).sum();

    let is_refactoring = high_churn_files.len() >= 2
        || (total_added > 50
            && total_removed > 50
            && (total_removed as f32 / total_added as f32) > 0.4);

    RefactoringSignals {
        is_refactoring,
        description: if high_churn_files.len() >= 2 {
            format!(
                "{} files significantly restructured",
                high_churn_files.len()
            )
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

fn determine_body_requirement(
    patterns: &[Pattern],
    complexity: f32,
    _diff_info: &DiffInfo,
) -> bool {
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
    if patterns
        .iter()
        .any(|p| matches!(p.pattern_type, PatternType::CrossLayerChange))
    {
        return true;
    }

    // multiple features added
    let feature_count = patterns
        .iter()
        .filter(|p| {
            matches!(
                p.pattern_type,
                PatternType::FeatureAddition | PatternType::NewFilePattern
            )
        })
        .count();
    if feature_count >= 2 {
        return true;
    }

    // architectural or interface changes
    if patterns.iter().any(|p| {
        matches!(
            p.pattern_type,
            PatternType::ArchitecturalShift | PatternType::InterfaceEvolution
        )
    }) {
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
        grouped
            .entry(pattern.pattern_type.clone())
            .or_default()
            .push(pattern);
    }

    // generate bullets based on pattern types
    for (pattern_type, group) in grouped {
        match pattern_type {
            PatternType::NewFilePattern => {
                let files = group
                    .iter()
                    .flat_map(|p| &p.files_affected)
                    .collect::<Vec<_>>();
                bullets.push(format!(
                    "add {} new file{}: {}",
                    files.len(),
                    if files.len() > 1 { "s" } else { "" },
                    summarize_files(&files)
                ));
            }
            PatternType::FeatureAddition => {
                for pattern in group {
                    bullets.push(pattern.description.clone());
                }
            }
            PatternType::CrossLayerChange => {
                bullets.push("implement changes across multiple application layers".to_string());
            }
            PatternType::RefactoringPattern => {
                bullets.push("refactor code for better maintainability".to_string());
            }
            PatternType::ConfigurationDrift => {
                bullets.push("update configuration settings".to_string());
            }
            PatternType::InterfaceEvolution => {
                bullets.push("modify api contracts or interfaces".to_string());
            }
            PatternType::Deprecation => {
                bullets.push("mark outdated functionality for removal".to_string());
                bullets.push("provide migration guidance for deprecated features".to_string());
            }
            PatternType::SecurityFix => {
                bullets.push("address security vulnerability".to_string());
                bullets.push("strengthen authentication and authorization".to_string());
                bullets.push("improve input validation and sanitisation".to_string());
            }
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
        files
            .iter()
            .map(|f| {
                std::path::Path::new(f)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(f)
            })
            .collect::<Vec<_>>()
            .join(", ")
    } else {
        format!(
            "{} and {} more",
            files
                .iter()
                .take(2)
                .map(|f| {
                    std::path::Path::new(f)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(f)
                })
                .collect::<Vec<_>>()
                .join(", "),
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
            }
            PatternType::BugFixPattern => {
                *type_scores.entry("fix").or_default() += pattern.impact * 1.5; // prioritise fix
            }
            PatternType::RefactoringPattern => {
                *type_scores.entry("refactor").or_default() += pattern.impact;
            }
            PatternType::TestEvolution => {
                *type_scores.entry("test").or_default() += pattern.impact;
            }
            PatternType::DocumentationUpdate => {
                *type_scores.entry("docs").or_default() += pattern.impact;
            }
            PatternType::PerformanceTuning => {
                *type_scores.entry("perf").or_default() += pattern.impact;
            }
            PatternType::ConfigurationDrift | PatternType::DependencyUpdate => {
                *type_scores.entry("build").or_default() += pattern.impact * 0.8;
            }
            PatternType::StyleNormalization => {
                *type_scores.entry("style").or_default() += pattern.impact;
            }
            PatternType::CiChange => {
                *type_scores.entry("ci").or_default() += pattern.impact;
            }
            PatternType::Deprecation => {
                *type_scores.entry("feat").or_default() += pattern.impact * 1.2; // deprecation often indicates API evolution/features
            }
            PatternType::SecurityFix => {
                *type_scores.entry("fix").or_default() += pattern.impact * 2.0; // security fixes are high priority fixes
            }
            _ => {
                *type_scores.entry("chore").or_default() += pattern.impact * 0.5;
                // Use chore as fallback for uncategorised
            }
        }
    }

    let commit_type = if type_scores.is_empty() {
        "chore".to_string() // Fallback to chore if no patterns detected
    } else {
        type_scores
            .iter()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(t, _)| t.to_string())
            .unwrap_or_else(|| "chore".to_string())
    };

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
    let extensions: HashSet<_> = diff_info
        .files
        .iter()
        .filter_map(|f| std::path::Path::new(&f.path).extension())
        .map(|e| e.to_string_lossy().to_string())
        .collect();

    // map common extensions to scopes
    if extensions
        .iter()
        .any(|e| ["ts", "js", "tsx", "jsx"].contains(&e.as_str()))
    {
        if extensions
            .iter()
            .any(|e| ["css", "scss", "sass"].contains(&e.as_str()))
        {
            return Some("ui".to_string());
        }
        return Some("frontend".to_string());
    }

    if extensions
        .iter()
        .any(|e| ["cs", "java", "py", "go"].contains(&e.as_str()))
    {
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
    prompt.push_str(&format!(
        "📊 COMMIT COMPLEXITY: {:.1}/5.0 - {}\n",
        intelligence.complexity_score,
        if intelligence.complexity_score < 1.5 {
            "simple"
        } else if intelligence.complexity_score < 2.5 {
            "moderate"
        } else {
            "complex"
        }
    ));

    // body requirement (crystal clear)
    if intelligence.requires_body {
        prompt.push_str("\n🔴 BODY REQUIRED - this commit is too complex for a single line.\n");
        prompt.push_str("the commit message MUST include a body with bullet points.\n\n");
    } else {
        prompt.push_str("\n✅ SINGLE LINE - this is a focused change, no body needed.\n\n");
    }

    // language context - infer dominant language from file extensions
    let dominant_language = infer_dominant_language(diff_info);
    if dominant_language != "Unknown" {
        prompt.push_str(&format!(
            "\n🌐 LANGUAGE CONTEXT: Primarily {} code - tailor examples accordingly.\n",
            dominant_language
        ));
    }

    // detected patterns
    prompt.push_str("🔍 DETECTED PATTERNS:\n");
    for pattern in &intelligence.detected_patterns {
        prompt.push_str(&format!(
            "- {}: {} (impact: {:.1})\n",
            format_pattern_type(&pattern.pattern_type),
            pattern.description,
            pattern.impact
        ));
    }
    prompt.push('\n');

    // suggested structure (made advisory)
    prompt.push_str("📝 RECOMMENDED COMMIT STRUCTURE (choose best fit based on code analysis):\n");
    prompt.push_str(&format!("type: {}\n", intelligence.commit_type_hint));
    if let Some(scope) = &intelligence.scope_hint {
        prompt.push_str(&format!("scope: {}\n", scope));
    }
    prompt.push_str("\nRATIONALE FOR RECOMMENDATION:\n");
    prompt.push_str(&format!(
        "- Type '{}' suggested based on dominant patterns: {}\n",
        intelligence.commit_type_hint,
        intelligence
            .detected_patterns
            .iter()
            .map(|p| format_pattern_type(&p.pattern_type))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    if let Some(scope) = &intelligence.scope_hint {
        prompt.push_str(&format!(
            "- Scope '{}' based on affected files: {}\n",
            scope,
            diff_info
                .files
                .iter()
                .take(3)
                .map(|f| f.path.clone())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    prompt.push_str("\nALLOWED TYPES: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert\n\n");

    // if body required, provide bullet suggestions
    if intelligence.requires_body && !intelligence.suggested_bullets.is_empty() {
        prompt.push_str("📌 SUGGESTED BULLET POINTS FOR BODY:\n");
        for bullet in &intelligence.suggested_bullets {
            prompt.push_str(&format!("- {}\n", bullet));
        }
        prompt.push('\n');
    }

    // send actual diff content for AI to understand
    prompt.push_str("📁 ACTUAL CODE CHANGES:\n");
    prompt.push_str(&diff_info.summary);
    prompt.push('\n');

    // include actual diff snippets for important files only
    if !diff_info.files.is_empty() {
        prompt.push_str("\n🔍 DIFF CONTENT (for context):\n");

        // filter and prioritise files
        let important_files = get_important_files_for_diff(&diff_info.files);
        let mut total_diff_lines = 0;
        const MAX_TOTAL_DIFF_LINES: usize = 3000; // increased for more code context

        for (i, file) in important_files.iter().enumerate() {
            if i >= 15 || total_diff_lines >= MAX_TOTAL_DIFF_LINES {
                break;
            }

            prompt.push_str(&format!(
                "\n--- {} (+{} -{}) ---\n",
                file.path, file.added_lines, file.removed_lines
            ));

            // dynamic line limits based on file importance and size
            let lines_to_include =
                calculate_diff_lines_for_file(file, total_diff_lines, MAX_TOTAL_DIFF_LINES);
            let meaningful_diff =
                extract_meaningful_diff_lines(&file.diff_content, lines_to_include);

            if !meaningful_diff.is_empty() {
                prompt.push_str(&meaningful_diff);
                prompt.push('\n');
                total_diff_lines += meaningful_diff.lines().count();
            } else {
                prompt.push_str(&format!(
                    "Large diff with {} additions, {} deletions\n",
                    file.added_lines, file.removed_lines
                ));
            }
        }

        let skipped_files = diff_info.files.len() - important_files.len();
        if skipped_files > 0 {
            prompt.push_str(&format!(
                "\n... and {} more files (auto-generated/less important)\n",
                skipped_files
            ));
        }
    }
    prompt.push('\n');

    // provide specific examples based on detected patterns (tailored to recommended type/scope and language)
    prompt.push_str("✨ LANGUAGE-TAILORED EXAMPLES FOR THIS TYPE OF CHANGE:\n");
    if intelligence.requires_body {
        // provide examples that match the detected patterns and language
        let has_new_files = intelligence
            .detected_patterns
            .iter()
            .any(|p| matches!(p.pattern_type, PatternType::NewFilePattern));
        let has_refactoring = intelligence
            .detected_patterns
            .iter()
            .any(|p| matches!(p.pattern_type, PatternType::RefactoringPattern));
        let has_features = intelligence
            .detected_patterns
            .iter()
            .any(|p| matches!(p.pattern_type, PatternType::FeatureAddition));

        prompt.push_str("```\n");
        let scope_str = intelligence
            .scope_hint
            .as_ref()
            .map_or("".to_string(), |s| format!("({})", s));

        // tailor examples based on language and patterns  
        if dominant_language.starts_with("Mixed") {
            // handle mixed language repositories
            prompt.push_str(&format!(
                "{}{}: implement cross-platform functionality\n\n",
                intelligence.commit_type_hint, scope_str
            ));
            prompt.push_str("- Add shared logic between frontend and backend\n");
            prompt.push_str("- Implement consistent error handling patterns\n");
            prompt.push_str("- Create unified configuration management\n");
            prompt.push_str("- Ensure compatibility across language boundaries\n");
        } else {
            match dominant_language.as_str() {
                "Rust" if has_new_files && has_features => {
                prompt.push_str(&format!(
                    "{}{}: implement pattern detection for commit analysis\n\n",
                    intelligence.commit_type_hint, scope_str
                ));
                prompt.push_str("- Add PatternType enum with deprecation detection\n");
                prompt.push_str("- Implement detect_universal_patterns function\n");
                prompt.push_str("- Enhance Result error handling with anyhow context\n");
                prompt.push_str("- Add comprehensive file type classification\n");
            }
            "JavaScript/TypeScript" if has_new_files && has_features => {
                prompt.push_str(&format!(
                    "{}{}: implement responsive ui components with dark mode\n\n",
                    intelligence.commit_type_hint, scope_str
                ));
                prompt.push_str("- Add responsive FlexContainer with media queries\n");
                prompt.push_str("- Implement theme provider for dark/light modes\n");
                prompt.push_str("- Create reusable Button and Input components\n");
                prompt.push_str("- Fix z-index issues in modal overlays\n");
            }
            "Python" if has_new_files && has_features => {
                prompt.push_str(&format!(
                    "{}{}: implement data processing pipeline with validation\n\n",
                    intelligence.commit_type_hint, scope_str
                ));
                prompt.push_str("- Add DataProcessor class with async methods\n");
                prompt.push_str("- Implement pydantic models for input validation\n");
                prompt.push_str("- Create pipeline orchestration with error handling\n");
                prompt.push_str("- Add comprehensive unit tests with pytest\n");
            }
            _ if has_refactoring => {
                prompt.push_str(&format!(
                    "{}{}: restructure codebase for better maintainability\n\n",
                    intelligence.commit_type_hint, scope_str
                ));
                prompt.push_str("- Extract shared functionality into utilities\n");
                prompt.push_str("- Improve separation of concerns across modules\n");
                prompt.push_str("- Consolidate duplicate logic patterns\n");
            }
                _ => {
                    prompt.push_str(&format!(
                        "{}{}: {}\n\n",
                        intelligence.commit_type_hint, scope_str, "describe the main change briefly"
                    ));
                    prompt.push_str("- Explain first major change with technical specifics\n");
                    prompt.push_str("- Describe second significant modification\n");
                    prompt.push_str("- Note any important architectural decisions\n");
                }
            }
        }
        prompt.push_str("```\n\n");
    } else {
        // simple single-line examples tailored to language
        prompt.push_str("```\n");
        let scope_str = intelligence
            .scope_hint
            .as_ref()
            .map_or("".to_string(), |s| format!("({})", s));

        if dominant_language.starts_with("Mixed") {
            // mixed language examples
            match intelligence.commit_type_hint.as_str() {
                "feat" => {
                    prompt.push_str(&format!(
                        "feat{}: add cross-platform authentication service\n",
                        scope_str
                    ));
                    prompt.push_str(&format!(
                        "feat{}: implement shared config management\n",
                        scope_str
                    ));
                }
                "fix" => {
                    prompt.push_str(&format!(
                        "fix{}: resolve data synchronisation between frontend and backend\n",
                        scope_str
                    ));
                }
                _ => {
                    prompt.push_str(&format!(
                        "{}{}: update cross-platform functionality\n",
                        intelligence.commit_type_hint, scope_str
                    ));
                }
            }
        } else {
            match (
                intelligence.commit_type_hint.as_str(),
                dominant_language.as_str(),
            ) {
            ("feat", "Rust") => {
                prompt.push_str(&format!(
                    "feat{}: add deprecation detection in pattern analysis\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}: implement Result-based error propagation\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}!: introduce breaking changes to public api\n",
                    scope_str
                ));
            }
            ("feat", "JavaScript/TypeScript") => {
                prompt.push_str(&format!(
                    "feat{}: add dark mode toggle with context provider\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}: implement responsive navigation component\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}!: migrate to new routing architecture\n",
                    scope_str
                ));
            }
            ("feat", "Python") => {
                prompt.push_str(&format!(
                    "feat{}: add async data validation pipeline\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}: implement pydantic model serialisation\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "feat{}!: update to python 3.12 type annotations\n",
                    scope_str
                ));
            }
            ("fix", _) => {
                prompt.push_str(&format!(
                    "fix{}: resolve memory leak in diff processing\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "fix{}: handle edge case in api response parsing\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "fix{}: prevent null pointer dereference\n",
                    scope_str
                ));
            }
            ("refactor", _) => {
                prompt.push_str(&format!(
                    "refactor{}: extract validation logic into utilities\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "refactor{}: simplify error handling patterns\n",
                    scope_str
                ));
                prompt.push_str(&format!(
                    "refactor{}: consolidate duplicate file processing\n",
                    scope_str
                ));
            }
            _ => {
                prompt.push_str(&format!(
                    "{}{}: {}\n",
                    intelligence.commit_type_hint,
                    scope_str,
                    "brief description of specific change"
                ));
            }
            }
        }
        prompt.push_str("```\n\n");
    }

    // clear instructions (encourage analysis)
    prompt.push_str("🎯 INSTRUCTIONS:\n");
    prompt.push_str("1. ANALYSE THE CODE DIFFS and patterns to choose the BEST type from the allowed list above. Use your reasoning to select the most appropriate one, even if it differs from the recommendation.\n");
    if intelligence.requires_body {
        prompt.push_str(
            "2. create a commit with type, optional scope, and description (under 72 chars)\n",
        );
        prompt.push_str("3. add a blank line\n");
        prompt.push_str("4. add a body with bullet points explaining the key changes\n");
        prompt.push_str("5. BE SPECIFIC: mention actual function names, modules, and purposes from the diff content\n");
        prompt.push_str(
            "6. ORGANISE BULLETS: major changes first, then features, then minor updates\n",
        );
        prompt.push_str("7. NO GENERIC COUNTS: instead of '15 functions', say 'add analyse_commit_intelligence function for pattern detection'\n");
        prompt.push_str("8. FOLLOW CONVENTIONAL COMMITS 1.0: use ! for breaking changes or BREAKING CHANGE: footer\n");
        prompt.push_str(
            "9. FOOTERS: use BREAKING CHANGE: (all caps) for breaking changes in footer\n",
        );
        prompt.push_str(
            "10. CAPITALISATION: bullet points start with capital letter, header stays lowercase\n",
        );
        prompt.push_str("11. focus on WHAT changed and WHY, not implementation details\n");
        prompt.push_str("12. use UK english spelling (optimisation, behaviour, etc.)\n");
    } else {
        prompt.push_str("2. create a single-line commit message\n");
        prompt.push_str("3. format: <type>(<scope>): <description>\n");
        prompt.push_str("4. description must be under 72 characters\n");
        prompt.push_str("5. NO BODY - just the single line\n");
        prompt.push_str("6. use UK english spelling\n");
    }

    prompt.push_str("\ngenerate the commit message now:\n");

    prompt
}

/// get important files for diff context, excluding auto-generated and prioritising by importance
fn get_important_files_for_diff(
    files: &[crate::git::ModifiedFile],
) -> Vec<&crate::git::ModifiedFile> {
    let mut important_files: Vec<&crate::git::ModifiedFile> = files
        .iter()
        .filter(|f| !is_auto_generated_or_boring_file(&f.path))
        .collect();

    // sort by importance (source code files first, then tests, then config)
    important_files.sort_by(|a, b| {
        let a_priority = get_file_priority(&a.path);
        let b_priority = get_file_priority(&b.path);
        a_priority.cmp(&b_priority)
    });

    important_files
}

/// check if file should be excluded from diff (auto-generated, minified, etc.)
fn is_auto_generated_or_boring_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // auto-generated files
    if path_lower.contains("node_modules")
        || path_lower.contains("target/")
        || path_lower.contains("build/")
        || path_lower.contains("dist/")
        || path_lower.contains(".git/")
        || path_lower.contains("__pycache__")
    {
        return true;
    }

    // lock files and auto-generated manifests
    if file_name.ends_with(".lock")
        || file_name.ends_with("-lock.json")
        || file_name == "package-lock.json"
        || file_name == "yarn.lock"
        || file_name == "cargo.lock"
        || file_name == "go.sum"
        || file_name == "poetry.lock"
    {
        return true;
    }

    // minified files
    if file_name.contains(".min.")
        || file_name.ends_with(".min.js")
        || file_name.ends_with(".min.css")
    {
        return true;
    }

    // generated docs and assets
    if path_lower.contains("/generated/")
        || path_lower.contains("/auto/")
        || path_lower.contains("/.generated")
        || file_name.starts_with("generated_")
    {
        return true;
    }

    // binary and media files
    if file_name.ends_with(".png")
        || file_name.ends_with(".jpg")
        || file_name.ends_with(".jpeg")
        || file_name.ends_with(".gif")
        || file_name.ends_with(".ico")
        || file_name.ends_with(".pdf")
        || file_name.ends_with(".exe")
        || file_name.ends_with(".dll")
        || file_name.ends_with(".so")
    {
        return true;
    }

    false
}

/// get priority for file ordering (lower number = higher priority)
fn get_file_priority(path: &str) -> u8 {
    let path_lower = path.to_lowercase();
    let file_name = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();

    // highest priority: core source files
    if path_lower.contains("/src/")
        && (file_name.ends_with(".rs")
            || file_name.ends_with(".ts")
            || file_name.ends_with(".js")
            || file_name.ends_with(".py")
            || file_name.ends_with(".go")
            || file_name.ends_with(".java")
            || file_name.ends_with(".cpp")
            || file_name.ends_with(".c"))
    {
        return 1;
    }

    // high priority: lib files and main modules
    if file_name == "lib.rs"
        || file_name == "main.rs"
        || file_name == "index.ts"
        || file_name == "index.js"
        || file_name == "main.py"
        || file_name == "__init__.py"
    {
        return 2;
    }

    // medium priority: configuration files
    if file_name.ends_with(".toml")
        || file_name.ends_with(".yaml")
        || file_name.ends_with(".yml")
        || file_name.ends_with(".json")
        || file_name == "package.json"
        || file_name == "cargo.toml"
        || file_name == "pyproject.toml"
    {
        return 3;
    }

    // medium-low priority: test files
    if path_lower.contains("/test")
        || path_lower.contains("/spec")
        || file_name.contains("test")
        || file_name.contains("spec")
    {
        return 4;
    }

    // low priority: documentation
    if file_name.ends_with(".md")
        || file_name.ends_with(".txt")
        || path_lower.contains("/docs/")
        || path_lower.contains("/doc/")
    {
        return 5;
    }

    // lowest priority: everything else
    6
}

/// calculate how many diff lines to include for this file
fn calculate_diff_lines_for_file(
    file: &crate::git::ModifiedFile,
    used_lines: usize,
    max_total: usize,
) -> usize {
    let remaining_budget = max_total.saturating_sub(used_lines);
    let file_priority = get_file_priority(&file.path);

    // allocate more lines to higher priority files (much more generous)
    let base_allocation = match file_priority {
        1 => 150, // core source files get lots of lines
        2 => 100, // lib files
        3 => 50,  // config files
        4 => 30,  // test files
        5 => 20,  // docs
        _ => 10,  // everything else
    };

    // but never exceed remaining budget
    std::cmp::min(base_allocation, remaining_budget)
}

/// extract the most meaningful lines from a diff for AI context
fn extract_meaningful_diff_lines(diff_content: &str, max_lines: usize) -> String {
    let mut meaningful_lines = Vec::new();
    let mut line_count = 0;

    for line in diff_content.lines() {
        if line_count >= max_lines {
            break;
        }

        let trimmed = line.trim();

        // skip empty lines and boring changes
        if trimmed.is_empty()
            || trimmed.starts_with("@@")
            || trimmed.starts_with("+++")
            || trimmed.starts_with("---")
        {
            continue;
        }

        // include all additions and important deletions
        if line.starts_with('+') || line.starts_with('-') {
            // prioritise function definitions, struct definitions, important logic
            if is_important_line(trimmed) {
                meaningful_lines.push(line.to_string());
                line_count += 1;
            } else if meaningful_lines.len() < (max_lines * 3) / 4 {
                // include more context lines now that we have more budget
                meaningful_lines.push(line.to_string());
                line_count += 1;
            }
        }
    }

    if meaningful_lines.is_empty() {
        return String::new();
    }

    // add truncation notice if we hit the limit
    if line_count >= max_lines {
        meaningful_lines.push("... (diff truncated for brevity)".to_string());
    }

    meaningful_lines.join("\n")
}

/// check if a diff line contains important code changes
fn is_important_line(line: &str) -> bool {
    let line_clean = line.trim_start_matches(['+', '-']).trim();

    // function definitions
    if line_clean.contains("fn ")
        || line_clean.contains("function ")
        || line_clean.contains("def ")
        || line_clean.contains("func ")
    {
        return true;
    }

    // type definitions
    if line_clean.contains("struct ")
        || line_clean.contains("enum ")
        || line_clean.contains("class ")
        || line_clean.contains("interface ")
        || line_clean.contains("type ")
    {
        return true;
    }

    // imports/exports (show dependencies)
    if line_clean.starts_with("use ")
        || line_clean.starts_with("import ")
        || line_clean.starts_with("from ")
        || line_clean.starts_with("export ")
    {
        return true;
    }

    // pub declarations (public API changes)
    if line_clean.starts_with("pub ") {
        return true;
    }

    // constants and configuration
    if line_clean.contains("const ")
        || line_clean.contains("static ")
        || line_clean.contains("config")
        || line_clean.contains("Config")
    {
        return true;
    }

    // comments explaining what's happening
    if line_clean.starts_with("//") || line_clean.starts_with("#") || line_clean.starts_with("/*") {
        return true;
    }

    false
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
        PatternType::CiChange => "ci changes",
        PatternType::Deprecation => "deprecation",
        PatternType::SecurityFix => "security fix",
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
pub fn select_model_for_complexity(
    intelligence: &CommitIntelligence,
    debug: bool,
    config: &Config,
) -> String {
    let (model, reason) = if intelligence.complexity_score < 1.5 {
        (
            &config.models.fast,
            "simple commit detected - using fast model",
        )
    } else if intelligence.complexity_score < 2.5 {
        (
            &config.models.thinking,
            "medium complexity commit - using thinking model",
        )
    } else {
        (
            &config.models.thinking,
            "complex commit detected - using thinking model",
        )
    };

    if debug {
        println!("🤖 smart model selection: {} ({})", reason, model);
    }

    model.to_string()
}

/// get available models for manual selection
pub fn get_available_models(config: &Config) -> Vec<(&str, &str)> {
    config
        .models
        .available
        .iter()
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

            // collect all lines until we hit something that looks like explanation
            if trimmed.is_empty() {
                commit_lines.push("".to_string()); // preserve blank lines
            } else {
                commit_lines.push(trimmed.to_string());
            }
        }
    }

    // if we found a multi-line commit, return it
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
    let fallback = response
        .lines()
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
    let valid_types = [
        "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore",
        "revert",
    ];

    if line.contains(':') {
        let before_colon = line.split(':').next().unwrap_or("").trim();

        // handle type(scope) or type[scope] pattern, and type! for breaking changes
        let type_part = if before_colon.contains('(') {
            before_colon.split('(').next().unwrap_or("").trim()
        } else if before_colon.contains('[') {
            before_colon.split('[').next().unwrap_or("").trim()
        } else {
            before_colon
        };

        // remove ! for breaking changes to get the base type
        let type_part = type_part.trim_end_matches('!');

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
                || !scope
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_'))
        {
            return Err(anyhow::anyhow!(
                "invalid scope '{}', must be a noun (alphanumeric, hyphens, or underscores only)",
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

    let first_char = description.chars().next().unwrap();
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
- CAPITALISE first word of each bullet point (e.g., "Add new feature", "Implement function")
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

fn detect_doc_files(analysis: &FileAnalysis) -> Vec<String> {
    let doc_indicators = [".md", ".rst", ".txt", "README", "docs/", "documentation/"];
    analysis
        .new_files
        .iter()
        .chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            doc_indicators.iter().any(|ind| lower.contains(ind))
        })
        .cloned()
        .collect()
}

fn detect_style_changes(diff_info: &DiffInfo) -> bool {
    let total_changes: usize = diff_info
        .files
        .iter()
        .map(|f| f.added_lines + f.removed_lines)
        .sum();
    let meaningful_changes: usize = diff_info
        .files
        .iter()
        .map(|f| {
            f.diff_content
                .lines()
                .filter(|l| {
                    let trim = l.trim();
                    !trim.is_empty()
                        && !trim.starts_with("//")
                        && !trim.starts_with("/*")
                        && !trim.starts_with("#")
                })
                .count()
        })
        .sum();
    total_changes > 0 && (meaningful_changes as f32 / total_changes as f32) < 0.3
}

fn detect_ci_files(analysis: &FileAnalysis) -> Vec<String> {
    let ci_indicators = [
        ".github/workflows",
        ".gitlab-ci.yml",
        "jenkinsfile",
        ".circleci/",
        "azure-pipelines.yml",
        "bitbucket-pipelines.yml",
    ];
    analysis
        .new_files
        .iter()
        .chain(&analysis.modified_files)
        .filter(|f| {
            let lower = f.to_lowercase();
            ci_indicators.iter().any(|ind| lower.contains(ind))
        })
        .cloned()
        .collect()
}

/// infer programming languages from file extensions (supports multi-language repos)
fn infer_dominant_language(diff_info: &DiffInfo) -> String {
    let mut lang_count: std::collections::HashMap<String, usize> = std::collections::HashMap::new();

    for file in &diff_info.files {
        if let Some(ext) = std::path::Path::new(&file.path).extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            let language = match ext_str.as_str() {
                "rs" => "Rust",
                "js" | "jsx" | "ts" | "tsx" | "vue" | "svelte" => "JavaScript/TypeScript",
                "py" | "pyi" => "Python",
                "go" => "Go",
                "java" | "kt" => "Java/Kotlin",
                "cpp" | "cc" | "cxx" | "c" | "h" | "hpp" => "C/C++",
                "cs" => "C#",
                "rb" => "Ruby",
                "php" => "PHP",
                "swift" => "Swift",
                "dart" => "Dart",
                "scala" => "Scala",
                "hs" => "Haskell",
                "clj" | "cljs" => "Clojure",
                "ex" | "exs" => "Elixir",
                "cr" => "Crystal",
                "nim" => "Nim",
                "zig" => "Zig",
                _ => continue, // skip unknown extensions
            };
            *lang_count.entry(language.to_string()).or_insert(0) += 1;
        }
    }

    if lang_count.is_empty() {
        return "Unknown".to_string();
    }

    // sort languages by frequency
    let mut sorted_langs: Vec<_> = lang_count.iter().collect();
    sorted_langs.sort_by(|a, b| b.1.cmp(a.1));

    // if multiple significant languages (more than 20% of total), show as mixed
    let total_files = sorted_langs.iter().map(|(_, count)| *count).sum::<usize>();
    let significant_threshold = (total_files as f32 * 0.2).ceil() as usize;

    let significant_langs: Vec<String> = sorted_langs
        .iter()
        .filter(|(_, count)| **count >= significant_threshold)
        .map(|(lang, _)| (*lang).clone())
        .collect();

    if significant_langs.len() > 1 {
        format!("Mixed ({})", significant_langs.join(", "))
    } else {
        sorted_langs[0].0.clone()
    }
}
