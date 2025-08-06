// prompt construction module - builds prompts for ai interaction

use super::intelligence::CommitIntelligence;
use super::patterns::PatternType;
use crate::git::DiffInfo;

/// construct intelligent prompt using commit analysis
pub fn construct_intelligent_prompt(
    diff_info: &DiffInfo,
    intelligence: &CommitIntelligence,
) -> String {
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

    // body requirement
    if intelligence.requires_body {
        prompt.push_str("\n🔴 BODY REQUIRED - this commit is too complex for a single line.\n");
        prompt.push_str("the commit message MUST include a body with bullet points.\n\n");
    } else {
        prompt.push_str("\n✅ SINGLE LINE - this is a focused change, no body needed.\n\n");
    }

    // language context
    let dominant_language = infer_dominant_language(diff_info);
    if dominant_language != "Unknown" {
        prompt.push_str(&format!(
            "\n🌐 LANGUAGE CONTEXT: Primarily {dominant_language} code - tailor examples accordingly.\n"
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

    // change context
    let subsystem = detect_subsystem(diff_info);
    prompt.push_str("🔧 CHANGE CONTEXT:\n");
    prompt.push_str(&format!("- Primary subsystem affected: {subsystem}\n"));
    prompt.push_str("- File purposes:\n");
    for (i, file) in diff_info.files.iter().enumerate() {
        if i >= 5 {
            break;
        }
        let purpose = get_file_purpose(&file.path);
        prompt.push_str(&format!("  * {} → {}\n", file.path, purpose));
    }
    prompt.push('\n');

    // recommended structure
    prompt.push_str("📝 RECOMMENDED COMMIT STRUCTURE (choose best fit based on code analysis):\n");
    prompt.push_str(&format!("type: {}\n", intelligence.commit_type_hint));
    prompt.push_str("scope: [DETERMINE FROM FILE PATHS AND CONTEXT ABOVE]\n");

    prompt.push_str(
        "\n📂 FILE PATHS AFFECTED (use these to determine the most appropriate scope):\n",
    );
    for (i, file) in diff_info.files.iter().enumerate() {
        if i >= 10 {
            prompt.push_str(&format!(
                "... and {} more files\n",
                diff_info.files.len() - 10
            ));
            break;
        }
        prompt.push_str(&format!("- {}\n", file.path));
    }

    prompt.push_str("\n🎯 SCOPE DETERMINATION GUIDELINES:\n");
    prompt.push_str("- analyse the file paths to identify the most specific, meaningful scope\n");
    prompt.push_str("- use the actual module, component, feature, or project folder name\n");
    prompt.push_str("- if files span multiple unrelated areas, omit the scope\n");
    prompt.push_str("- prefer specific scopes over generic ones (e.g., 'auth' not 'backend')\n");

    prompt.push_str("\n⚠️ COMMON MISTAKES TO AVOID:\n");
    prompt.push_str("- DON'T use 'security' just because validation is mentioned\n");
    prompt.push_str("- DON'T confuse commit message validation with input/data validation\n");
    prompt.push_str(
        "- DON'T use generic scopes like 'app', 'project', 'system', 'frontend', 'backend'\n",
    );
    prompt.push_str("- DON'T use file extensions as scopes\n");

    prompt.push_str("\n📋 FORMAT EXAMPLES (follow these EXACTLY):\n");
    prompt.push_str("✅ CORRECT formats:\n");
    prompt.push_str("  - fix(ai): improve validation logic\n");
    prompt.push_str("  - feat(auth,api): add oauth support\n");
    prompt.push_str("  - refactor: simplify error handling\n");
    prompt.push_str("❌ WRONG formats:\n");
    prompt.push_str("  - fix(ai, napi): improve validation ← NO SPACES after commas!\n");
    prompt.push_str("  - Fix(ai): improve validation ← type must be lowercase!\n");

    prompt.push_str(&format!(
        "\nRATIONALE FOR TYPE RECOMMENDATION:\n- Type '{}' suggested based on patterns: {}\n",
        intelligence.commit_type_hint,
        intelligence
            .detected_patterns
            .iter()
            .map(|p| format_pattern_type(&p.pattern_type))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    prompt.push_str("\nALLOWED TYPES: feat, fix, docs, style, refactor, perf, test, build, ci, chore, revert\n\n");

    // body suggestions if needed
    if intelligence.requires_body && !intelligence.suggested_bullets.is_empty() {
        prompt.push_str("📌 SUGGESTED BULLET POINTS FOR BODY:\n");
        for bullet in &intelligence.suggested_bullets {
            prompt.push_str(&format!("- {bullet}\n"));
        }
        prompt.push('\n');
    }

    // actual code changes
    prompt.push_str("📁 ACTUAL CODE CHANGES:\n");
    prompt.push_str(&diff_info.summary);
    prompt.push('\n');

    // include diff snippets
    if !diff_info.files.is_empty() {
        prompt.push_str("\n🔍 DIFF CONTENT (for context):\n");

        let important_files = get_important_files_for_diff(&diff_info.files);
        let mut total_diff_lines = 0;
        const MAX_TOTAL_DIFF_LINES: usize = 3000;

        for (i, file) in important_files.iter().enumerate() {
            if i >= 15 || total_diff_lines >= MAX_TOTAL_DIFF_LINES {
                break;
            }

            prompt.push_str(&format!(
                "\n--- {} (+{} -{}) ---\n",
                file.path, file.added_lines, file.removed_lines
            ));

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
                "\n... and {skipped_files} more files (auto-generated/less important)\n"
            ));
        }
    }
    prompt.push('\n');

    // provide examples
    add_language_tailored_examples(&mut prompt, intelligence, &dominant_language);

    // clear instructions
    prompt.push_str("🎯 INSTRUCTIONS:\n");
    prompt.push_str("1. ANALYSE THE CODE DIFFS and patterns to choose the BEST type from the allowed list above.\n");
    if intelligence.requires_body {
        prompt.push_str(
            "2. create a commit with type, optional scope, and description (under 72 chars)\n",
        );
        prompt.push_str("3. add a blank line\n");
        prompt.push_str("4. add a body with bullet points explaining the key changes\n");
        prompt.push_str("5. BE SPECIFIC: mention actual function names, modules, and purposes\n");
        prompt.push_str(
            "6. ORGANISE BULLETS: major changes first, then features, then minor updates\n",
        );
        prompt.push_str("7. FOLLOW CONVENTIONAL COMMITS 1.0: use ! for breaking changes\n");
        prompt.push_str(
            "8. CAPITALISATION: bullet points start with capital letter, header stays lowercase\n",
        );
        prompt.push_str("9. focus on WHAT changed and WHY, not implementation details\n");
        prompt.push_str("10. use UK english spelling (optimisation, behaviour, etc.)\n");
    } else {
        prompt.push_str("2. create a single-line commit message\n");
        prompt.push_str("3. format: <type>(<scope>): <description>\n");
        prompt.push_str("4. description must be under 72 characters\n");
        prompt.push_str("5. NO BODY - just the single line\n");
        prompt.push_str("6. use UK english spelling\n");
    }
    prompt.push('\n');

    prompt.push_str("generate the commit message now, with no additional commentary.\n");

    prompt
}

/// get system prompt based on intelligence
pub fn get_system_prompt(intelligence: &CommitIntelligence) -> &'static str {
    if intelligence.complexity_score > 3.0 {
        "you are an expert software engineer writing precise, detailed commit messages. analyse the code changes and generate a conventional commit message that fully explains complex architectural changes."
    } else if intelligence.requires_body {
        "you are a senior developer creating clear commit messages. generate a conventional commit with proper type, scope, and a body with bullet points explaining the changes."
    } else {
        "you are a developer writing concise commit messages. generate a single-line conventional commit message that clearly describes the change."
    }
}

/// extract meaningful diff lines
pub fn extract_meaningful_diff_lines(diff_content: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = diff_content.lines().collect();
    let mut selected_lines = Vec::new();
    let mut line_count = 0;

    // prioritise added lines and important changes
    for line in &lines {
        if line_count >= max_lines {
            break;
        }

        if is_important_line(line) {
            selected_lines.push(*line);
            line_count += 1;
        }
    }

    // fill remaining with context if needed
    if line_count < max_lines {
        for line in &lines {
            if line_count >= max_lines {
                break;
            }

            if !selected_lines.contains(line) && !line.trim().is_empty() {
                selected_lines.push(*line);
                line_count += 1;
            }
        }
    }

    selected_lines.join("\n")
}

/// check if a line is important for ai context
fn is_important_line(line: &str) -> bool {
    // skip empty lines and pure formatting
    if line.trim().is_empty() {
        return false;
    }

    // prioritise additions
    if line.starts_with('+') && !line.starts_with("+++") {
        let content = line.trim_start_matches('+').trim();

        // skip pure formatting lines
        if content == "{" || content == "}" || content == "(" || content == ")" {
            return false;
        }

        // skip import-only lines unless they're significant
        if content.starts_with("import ") || content.starts_with("use ") {
            return content.contains(',') || content.len() > 50;
        }

        return true;
    }

    // include removals for context
    if line.starts_with('-') && !line.starts_with("---") {
        let content = line.trim_start_matches('-').trim();
        // skip trivial removals
        return !(content == "{" || content == "}" || content.is_empty());
    }

    // include file markers
    line.starts_with("@@") || line.starts_with("diff --git")
}

/// format pattern type for display
pub fn format_pattern_type(pattern_type: &PatternType) -> &'static str {
    match pattern_type {
        PatternType::NewFilePattern => "new file",
        PatternType::MassModification => "mass modification",
        PatternType::CrossLayerChange => "cross-layer",
        PatternType::InterfaceEvolution => "interface evolution",
        PatternType::ArchitecturalShift => "architectural",
        PatternType::ConfigurationDrift => "configuration",
        PatternType::DependencyUpdate => "dependency",
        PatternType::RefactoringPattern => "refactoring",
        PatternType::FeatureAddition => "feature",
        PatternType::BugFixPattern => "bugfix",
        PatternType::TestEvolution => "test",
        PatternType::DocumentationUpdate => "documentation",
        PatternType::StyleNormalization => "style",
        PatternType::PerformanceTuning => "performance",
        PatternType::SecurityHardening => "security",
        PatternType::CiChange => "ci/cd",
        PatternType::Deprecation => "deprecation",
        PatternType::SecurityFix => "security fix",
    }
}

/// infer dominant language from file extensions
fn infer_dominant_language(diff_info: &DiffInfo) -> String {
    let mut lang_counts = std::collections::HashMap::new();

    for file in &diff_info.files {
        if let Some(ext) = file.path.split('.').next_back() {
            let lang = match ext {
                "rs" => "Rust",
                "js" | "jsx" => "JavaScript",
                "ts" | "tsx" => "TypeScript",
                "py" => "Python",
                "go" => "Go",
                "java" => "Java",
                "cs" => "C#",
                "cpp" | "cc" | "cxx" => "C++",
                "c" | "h" => "C",
                "rb" => "Ruby",
                "php" => "PHP",
                "swift" => "Swift",
                "kt" | "kts" => "Kotlin",
                _ => continue,
            };
            *lang_counts.entry(lang).or_insert(0) += 1;
        }
    }

    if lang_counts.is_empty() {
        return "Unknown".to_string();
    }

    if lang_counts.len() > 1 {
        let langs: Vec<_> = lang_counts.keys().copied().collect();
        return format!("Mixed ({})", langs.join(", "));
    }

    lang_counts
        .iter()
        .max_by_key(|(_, count)| *count)
        .map(|(lang, _)| lang.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}

/// detect subsystem from file paths
fn detect_subsystem(diff_info: &DiffInfo) -> String {
    let paths: Vec<String> = diff_info.files.iter().map(|f| f.path.clone()).collect();

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

/// get file purpose description
fn get_file_purpose(path: &str) -> String {
    let path_lower = path.to_lowercase();

    if path_lower.contains("test") || path_lower.contains("spec") {
        "test file".to_string()
    } else if path_lower.contains("config") || path_lower.contains("settings") {
        "configuration".to_string()
    } else if path_lower.contains("api") || path_lower.contains("endpoint") {
        "api endpoint".to_string()
    } else if path_lower.contains("model") || path_lower.contains("schema") {
        "data model".to_string()
    } else if path_lower.contains("util") || path_lower.contains("helper") {
        "utility functions".to_string()
    } else if path_lower.contains("component") || path_lower.contains("view") {
        "ui component".to_string()
    } else if path_lower.contains("service") {
        "service layer".to_string()
    } else if path_lower.contains("middleware") {
        "middleware".to_string()
    } else {
        "implementation".to_string()
    }
}

/// get important files for diff
fn get_important_files_for_diff(
    files: &[crate::git::ModifiedFile],
) -> Vec<&crate::git::ModifiedFile> {
    let mut sorted_files: Vec<_> = files.iter().collect();

    // sort by priority
    sorted_files.sort_by(|a, b| {
        let a_priority = get_file_priority(&a.path);
        let b_priority = get_file_priority(&b.path);
        b_priority.cmp(&a_priority)
    });

    // filter out auto-generated files
    sorted_files
        .into_iter()
        .filter(|f| !is_auto_generated_or_boring_file(&f.path))
        .collect()
}

/// check if file is auto-generated or boring
fn is_auto_generated_or_boring_file(path: &str) -> bool {
    let path_lower = path.to_lowercase();

    // lock files
    if path_lower.ends_with(".lock")
        || path_lower.ends_with("-lock.json")
        || path_lower.ends_with("-lock.yaml")
    {
        return true;
    }

    // generated files
    if path_lower.contains("generated")
        || path_lower.contains(".min.")
        || path_lower.contains("dist/")
        || path_lower.contains("build/")
        || path_lower.contains("target/")
    {
        return true;
    }

    // binary files
    if path_lower.ends_with(".png")
        || path_lower.ends_with(".jpg")
        || path_lower.ends_with(".gif")
        || path_lower.ends_with(".ico")
        || path_lower.ends_with(".pdf")
    {
        return true;
    }

    false
}

/// get file priority for diff inclusion
fn get_file_priority(path: &str) -> u8 {
    let path_lower = path.to_lowercase();

    // highest priority: core logic files
    if path_lower.contains("/src/") && !path_lower.contains("test") {
        return 10;
    }

    // high priority: api/service files
    if path_lower.contains("api") || path_lower.contains("service") {
        return 9;
    }

    // medium priority: models and config
    if path_lower.contains("model") || path_lower.contains("config") {
        return 7;
    }

    // low priority: tests
    if path_lower.contains("test") || path_lower.contains("spec") {
        return 5;
    }

    // lowest priority: docs
    if path_lower.ends_with(".md") || path_lower.ends_with(".txt") {
        return 3;
    }

    // default
    6
}

/// calculate diff lines for file
fn calculate_diff_lines_for_file(
    file: &crate::git::ModifiedFile,
    current_total: usize,
    max_total: usize,
) -> usize {
    let remaining = max_total.saturating_sub(current_total);
    let file_changes = file.added_lines + file.removed_lines;

    // dynamic allocation based on file importance and remaining space
    if file_changes < 50 {
        file_changes.min(remaining)
    } else if file_changes < 200 {
        (file_changes / 2).min(remaining).min(100)
    } else {
        50.min(remaining)
    }
}

/// add language-tailored examples to prompt
fn add_language_tailored_examples(
    prompt: &mut String,
    intelligence: &CommitIntelligence,
    language: &str,
) {
    prompt.push_str("✨ LANGUAGE-TAILORED EXAMPLES FOR THIS TYPE OF CHANGE:\n");

    if intelligence.requires_body {
        add_multi_line_examples(prompt, intelligence, language);
    } else {
        add_single_line_examples(prompt, intelligence, language);
    }
}

/// add multi-line commit examples
fn add_multi_line_examples(prompt: &mut String, intelligence: &CommitIntelligence, language: &str) {
    prompt.push_str("```\n");

    let scope_str = intelligence
        .scope_hint
        .as_ref()
        .map_or("".to_string(), |s| format!("({s})"));

    match language {
        lang if lang.starts_with("Mixed") => {
            prompt.push_str(&format!(
                "{}{}: implement cross-platform functionality\n\n",
                intelligence.commit_type_hint, scope_str
            ));
            prompt.push_str("- Add shared logic between frontend and backend\n");
            prompt.push_str("- Implement consistent error handling patterns\n");
            prompt.push_str("- Create unified configuration management\n");
        }
        "Rust" => {
            prompt.push_str(&format!(
                "{}{}: implement pattern detection for commit analysis\n\n",
                intelligence.commit_type_hint, scope_str
            ));
            prompt.push_str("- Add PatternType enum with deprecation detection\n");
            prompt.push_str("- Implement detect_universal_patterns function\n");
            prompt.push_str("- Enhance Result error handling with anyhow context\n");
        }
        "JavaScript" | "TypeScript" => {
            prompt.push_str(&format!(
                "{}{}: implement responsive ui components\n\n",
                intelligence.commit_type_hint, scope_str
            ));
            prompt.push_str("- Add responsive FlexContainer with media queries\n");
            prompt.push_str("- Implement theme provider for dark/light modes\n");
            prompt.push_str("- Create reusable Button and Input components\n");
        }
        _ => {
            prompt.push_str(&format!(
                "{}{}: describe the main change briefly\n\n",
                intelligence.commit_type_hint, scope_str
            ));
            prompt.push_str("- Explain first major change with technical specifics\n");
            prompt.push_str("- Describe second significant modification\n");
            prompt.push_str("- Note any important architectural decisions\n");
        }
    }

    prompt.push_str("```\n\n");
}

/// add single-line commit examples
fn add_single_line_examples(
    prompt: &mut String,
    intelligence: &CommitIntelligence,
    language: &str,
) {
    prompt.push_str("```\n");

    let scope_str = intelligence
        .scope_hint
        .as_ref()
        .map_or("".to_string(), |s| format!("({s})"));

    match (intelligence.commit_type_hint.as_str(), language) {
        ("feat", "Rust") => {
            prompt.push_str(&format!(
                "feat{scope_str}: add deprecation detection in pattern analysis\n"
            ));
            prompt.push_str(&format!(
                "feat{scope_str}: implement Result-based error propagation\n"
            ));
        }
        ("feat", "JavaScript") | ("feat", "TypeScript") => {
            prompt.push_str(&format!(
                "feat{scope_str}: add dark mode toggle with context provider\n"
            ));
            prompt.push_str(&format!(
                "feat{scope_str}: implement responsive navigation component\n"
            ));
        }
        ("fix", _) => {
            prompt.push_str(&format!(
                "fix{scope_str}: resolve memory leak in diff processing\n"
            ));
            prompt.push_str(&format!(
                "fix{scope_str}: handle edge case in api response parsing\n"
            ));
        }
        ("refactor", _) => {
            prompt.push_str(&format!(
                "refactor{scope_str}: extract validation logic into utilities\n"
            ));
            prompt.push_str(&format!(
                "refactor{scope_str}: simplify error handling patterns\n"
            ));
        }
        _ => {
            prompt.push_str(&format!(
                "{}{}: brief description of specific change\n",
                intelligence.commit_type_hint, scope_str
            ));
        }
    }

    prompt.push_str("```\n\n");
}
