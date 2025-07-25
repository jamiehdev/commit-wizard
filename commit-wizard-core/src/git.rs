use anyhow::{Context, Result};
use encoding_rs::Encoding;
use git2::{DiffFormat, DiffOptions, Repository};

/// information about a modified file in the git diff
pub struct ModifiedFile {
    pub path: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub diff_content: String,
    pub file_type: FileType,
    pub change_hints: Vec<ChangeHint>,
}

/// categorize files by their purpose
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum FileType {
    SourceCode,
    Test,
    Documentation,
    Config,
    Build,
    Other,
}

/// hints about the nature of changes
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ChangeHint {
    BugFix,
    ErrorHandling,
    Refactor,
    NewFeature,
    Performance,
    Documentation,
    Dependencies,
    NewFunction,
    NewStruct,
    NewEnum,
    NewModule,
    MajorAddition,
    MinorTweak,
}

/// overall diff information from the repository
pub struct DiffInfo {
    pub files: Vec<ModifiedFile>,
    pub summary: String,
}

/// get diff information from a git repository
pub fn get_diff_info(
    repo_path: &str,
    max_file_size: usize,
    max_files: usize,
    verbose: bool,
) -> Result<DiffInfo> {
    // open the repository
    let repo = Repository::discover(repo_path).context("failed to open git repository")?;

    // create diff options
    let mut diff_opts = DiffOptions::new();
    diff_opts.show_binary(false);
    diff_opts.include_untracked(true);
    diff_opts.recurse_untracked_dirs(true);

    let mut files = Vec::new();

    // check if repository has any commits
    let has_head = repo.head().is_ok();

    // get the diff between HEAD and the index (staged changes)
    if has_head {
        if verbose {
            println!("analysing staged changes (index vs HEAD)...");
        }

        if let Ok(head) = repo.head() {
            if let Ok(tree) = head.peel_to_tree() {
                let diff = repo.diff_tree_to_index(Some(&tree), None, Some(&mut diff_opts))?;
                process_diff(&diff, &mut files, max_file_size, max_files, verbose)?;
            }
        }
    } else if verbose {
        println!("no commits yet, analysing all staged files...");
    }

    // if the repository doesn't have commits yet, get all staged files
    if !has_head {
        // for new repos, we need to get all staged files in the index
        if let Ok(index) = repo.index() {
            for entry in index.iter() {
                let path_str = std::str::from_utf8(&entry.path).unwrap_or_default();

                // process each staged file that doesn't exist in files yet
                if !files.iter().any(|f| f.path == path_str) {
                    let path = std::path::Path::new(repo_path).join(path_str);

                    // skip binary or large files
                    if let Ok(metadata) = std::fs::metadata(&path) {
                        if metadata.len() > max_file_size as u64 {
                            if verbose {
                                println!(
                                    "skipping large file: {} ({} KB)",
                                    path_str,
                                    metadata.len() / 1024
                                );
                            }
                            continue;
                        }
                    }

                    // read file content for new files
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let line_count = content.lines().count();

                        if verbose {
                            println!("adding new file: {path_str} ({line_count} lines)");
                        }

                        // add file to the list
                        files.push(ModifiedFile {
                            path: path_str.to_string(),
                            added_lines: line_count,
                            removed_lines: 0,
                            diff_content: format!("+{content}"),
                            file_type: classify_file_type(path_str),
                            change_hints: analyse_change_hints(&content, true),
                        });

                        // limit number of files
                        if files.len() >= max_files {
                            break;
                        }
                    }
                }
            }
        }
    }

    // only check unstaged changes if no staged changes were found
    if files.is_empty() {
        if verbose {
            println!("no staged changes found, checking unstaged changes (working directory vs index)...");
        }

        if let Ok(diff) = repo.diff_index_to_workdir(None, Some(&mut diff_opts)) {
            process_diff(&diff, &mut files, max_file_size, max_files, verbose)?;
        }
    } else if verbose {
        println!("staged changes found, skipping unstaged changes...");
    }

    if files.is_empty() {
        return Err(anyhow::anyhow!("no changes detected in the repository"));
    }

    // build a summary of the changes
    let file_count = files.len();
    let total_additions: usize = files.iter().map(|f| f.added_lines).sum();
    let total_deletions: usize = files.iter().map(|f| f.removed_lines).sum();

    let mut summary = format!(
        "{} file{} changed, {} insertion{}, {} deletion{}",
        file_count,
        if file_count == 1 { "" } else { "s" },
        total_additions,
        if total_additions == 1 { "" } else { "s" },
        total_deletions,
        if total_deletions == 1 { "" } else { "s" }
    );

    // generate detailed file breakdown for summary
    summary.push_str("\n\nfile breakdown:\n");
    for file in &files {
        let change_type = if file.removed_lines == 0 && file.added_lines > 5 {
            " (new file)"
        } else if file.added_lines > file.removed_lines * 2 {
            " (major additions)"
        } else if file.removed_lines > file.added_lines * 2 {
            " (major deletions)"
        } else {
            " (modified)"
        };

        summary.push_str(&format!(
            "  {} (+{}, -{}){}",
            file.path, file.added_lines, file.removed_lines, change_type
        ));

        // add specific code changes if available
        if !file.diff_content.is_empty() {
            let key_changes = extract_key_changes(&file.diff_content);
            if !key_changes.is_empty() {
                summary.push_str(&format!("\n    key changes: {key_changes}"));
            }
        }
        summary.push('\n');
    }

    Ok(DiffInfo { files, summary })
}

/// extract key changes from diff content to provide meaningful context
fn extract_key_changes(diff_content: &str) -> String {
    let mut changes = Vec::new();
    let added_lines: Vec<&str> = diff_content
        .lines()
        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
        .map(|l| l.trim_start_matches('+').trim())
        .filter(|l| !l.is_empty())
        .collect();

    // use simple string matching for much better performance instead of regex
    for line in &added_lines {
        let line_lower = line.to_lowercase();

        // check for function additions using fast string matching
        if (line.contains("fn ") && (line.contains("pub ") || line.contains("async ")))
            || line.contains("function ")
            || line.contains("def ")
        {
            // extract function name more efficiently
            if let Some(fn_pos) = line
                .find("fn ")
                .or(line.find("function "))
                .or(line.find("def "))
            {
                if let Some(name_start) = line[fn_pos..].find(' ') {
                    if let Some(name_end) = line[fn_pos + name_start + 1..]
                        .find(|c: char| c == '(' || c.is_whitespace())
                    {
                        let name =
                            &line[fn_pos + name_start + 1..fn_pos + name_start + 1 + name_end];
                        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                        {
                            changes.push(format!("add function {name}"));
                        }
                    }
                }
            }
        }

        // check for type additions using fast string matching
        if line.contains("struct ")
            || line.contains("enum ")
            || line.contains("class ")
            || line.contains("interface ")
        {
            changes.push("add type definition".to_string());
        }

        // check for imports using fast string matching
        if line_lower.contains("use ")
            || line_lower.contains("import ")
            || line_lower.contains("from ")
        {
            changes.push("add dependencies".to_string());
        }

        // check for configuration changes
        if line.contains("config") || line.contains("setting") {
            changes.push("modify configuration".to_string());
        }

        // check for error handling
        if line.contains("Error") || line.contains("Exception") || line.contains("Result") {
            changes.push("improve error handling".to_string());
        }

        // check for async/performance related
        if line.contains("async") || line.contains("await") || line.contains("cache") {
            changes.push("add async/performance features".to_string());
        }
    }

    // deduplicate and limit to most important changes
    let mut unique_changes: Vec<String> = changes
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    unique_changes.sort();
    unique_changes.truncate(3); // limit to top 3 changes

    unique_changes.join(", ")
}

/// check if there are any staged changes in the repository
pub fn has_staged_changes(repo_path: &str) -> Result<bool> {
    // open the repository
    let repo = Repository::discover(repo_path).context("failed to open git repository")?;

    // check if repository has any commits
    let has_head = repo.head().is_ok();

    // if the repository has no commits yet, check if there are any staged files
    if !has_head {
        if let Ok(index) = repo.index() {
            return Ok(!index.is_empty());
        }
        return Ok(false);
    }

    // check for staged changes by comparing HEAD to index
    if let Ok(head) = repo.head() {
        if let Ok(tree) = head.peel_to_tree() {
            let mut diff_opts = DiffOptions::new();
            diff_opts.show_binary(false);

            if let Ok(diff) = repo.diff_tree_to_index(Some(&tree), None, Some(&mut diff_opts)) {
                // if diff has any deltas, there are staged changes
                return Ok(diff.deltas().count() > 0);
            }
        }
    }

    Ok(false)
}

/// get a list of staged files for display
pub fn get_staged_files(repo_path: &str) -> Result<Vec<String>> {
    // open the repository
    let repo = Repository::discover(repo_path).context("failed to open git repository")?;

    let mut staged_files = Vec::new();

    // check if repository has any commits
    let has_head = repo.head().is_ok();

    // if the repository has no commits yet, get all staged files
    if !has_head {
        if let Ok(index) = repo.index() {
            for entry in index.iter() {
                if let Ok(path) = std::str::from_utf8(&entry.path) {
                    staged_files.push(path.to_string());
                }
            }
        }
        return Ok(staged_files);
    }

    // check for staged changes by comparing HEAD to index
    if let Ok(head) = repo.head() {
        if let Ok(tree) = head.peel_to_tree() {
            let mut diff_opts = DiffOptions::new();
            diff_opts.show_binary(false);

            if let Ok(diff) = repo.diff_tree_to_index(Some(&tree), None, Some(&mut diff_opts)) {
                diff.foreach(
                    &mut |delta, _| {
                        if let Some(path) = delta.new_file().path() {
                            staged_files.push(path.to_string_lossy().to_string());
                        }
                        true
                    },
                    None,
                    None,
                    None,
                )?;
            }
        }
    }

    Ok(staged_files)
}

/// process a diff to extract file information (optimised with batching)
fn process_diff(
    diff: &git2::Diff,
    files: &mut Vec<ModifiedFile>,
    max_file_size: usize,
    max_files: usize,
    verbose: bool,
) -> Result<()> {
    // first pass: collect valid file paths (simplified approach)
    let mut valid_paths = Vec::new();

    // handle early termination gracefully when max_files is reached
    let result = diff.foreach(
        &mut |delta, _| {
            let path = match delta.new_file().path() {
                Some(path) => path.to_string_lossy().to_string(),
                None => return true,
            };

            // skip binary files
            if delta.new_file().is_binary() || delta.old_file().is_binary() {
                if verbose {
                    println!("skipping binary file: {path}");
                }
                return true;
            }

            // skip files larger than max_file_size
            let file_size = match delta.new_file().size() {
                0 => delta.old_file().size(), // file might have been deleted
                size => size,
            };

            if file_size > max_file_size as u64 {
                if verbose {
                    println!("skipping large file: {} ({} KB)", path, file_size / 1024);
                }
                return true;
            }

            // stop if we've reached max_files
            if valid_paths.len() >= max_files {
                return false; // stop processing
            }

            valid_paths.push(path);
            true
        },
        None,
        None,
        None,
    );

    // handle git2 error code -7 (GIT_EUSER) as normal early termination
    match result {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::User => {
            // early termination due to max_files limit reached - this is expected
            if verbose {
                println!("reached maximum file limit ({max_files}), processing truncated");
            }
        }
        Err(e) => return Err(e.into()),
    }

    // second pass: initialise file entries for all valid paths
    for path in &valid_paths {
        files.push(ModifiedFile {
            path: path.clone(),
            added_lines: 0,
            removed_lines: 0,
            diff_content: String::new(),
            file_type: classify_file_type(path),
            change_hints: Vec::new(),
        });
    }

    // third pass: collect diff content efficiently
    diff.print(DiffFormat::Patch, |delta, _, line| {
        let path = match delta.new_file().path() {
            Some(path) => path.to_string_lossy().to_string(),
            None => return true,
        };

        // find the file entry (we know it exists from the second pass)
        if let Some(file_entry) = files.iter_mut().find(|f| f.path == path) {
            // track added and removed lines
            match line.origin() {
                '+' => file_entry.added_lines += 1,
                '-' => file_entry.removed_lines += 1,
                _ => {}
            }

            // append line to diff content (up to a reasonable size)
            if file_entry.diff_content.len() < 5000 {
                file_entry.diff_content.push(line.origin());
                file_entry
                    .diff_content
                    .push_str(&decode_line_content(line.content()));
            }
        }

        true
    })?;

    // batch process change hints for all files
    let change_hints: Vec<Vec<ChangeHint>> = files
        .iter()
        .map(|file| analyse_change_hints(&file.diff_content, false))
        .collect();

    // apply change hints in batch
    for (file, hints) in files.iter_mut().zip(change_hints) {
        file.change_hints = hints;
    }

    Ok(())
}

/// classify file type based on path and extension
fn classify_file_type(path: &str) -> FileType {
    let path_lower = path.to_lowercase();

    // test files (multi-language)
    if path_lower.contains("test")
        || path_lower.contains("spec")
        || path_lower.ends_with(".test.js")
        || path_lower.ends_with(".spec.js")
        || path_lower.ends_with(".test.ts")
        || path_lower.ends_with(".spec.ts")
        || path_lower.ends_with("tests.cs")
        || path_lower.ends_with("test.cs")
        || path_lower.contains("__tests__")
        || path_lower.contains(".tests/")
        || path_lower.ends_with("_test.py")
        || path_lower.ends_with("_test.rs")
    {
        return FileType::Test;
    }

    // documentation
    if path_lower.ends_with(".md")
        || path_lower.ends_with(".txt")
        || path_lower.ends_with(".rst")
        || path_lower.contains("readme")
        || path_lower.contains("doc")
        || path_lower.ends_with(".adoc")
    {
        return FileType::Documentation;
    }

    // config files (multi-language/platform)
    if path_lower.ends_with(".json")
        || path_lower.ends_with(".yaml")
        || path_lower.ends_with(".yml")
        || path_lower.ends_with(".xml")
        || path_lower.ends_with(".ini")
        || path_lower.ends_with(".conf")
        || path_lower.ends_with(".config")
        || path_lower.contains(".env")
        || path_lower.ends_with("appsettings.json")
        || path_lower.contains("web.config")
        || path_lower.ends_with(".toml")
    {
        return FileType::Config;
    }

    // build files (multi-language)
    if path_lower.contains("package.json")
        || path_lower.contains("package-lock.json")
        || path_lower.contains("yarn.lock")
        || path_lower.contains("pnpm-lock.yaml")
        || path_lower.ends_with(".csproj")
        || path_lower.ends_with(".sln")
        || path_lower.ends_with(".props")
        || path_lower.ends_with(".targets")
        || path_lower.contains("cargo")
        || path_lower.contains("makefile")
        || path_lower.contains("dockerfile")
        || path_lower.ends_with(".lock")
        || path_lower.contains("webpack")
        || path_lower.contains("vite.config")
        || path_lower.contains("rollup.config")
    {
        return FileType::Build;
    }

    // source code (multi-language)
    if path_lower.ends_with(".cs")
        || path_lower.ends_with(".vb")
        || path_lower.ends_with(".js")
        || path_lower.ends_with(".jsx")
        || path_lower.ends_with(".ts")
        || path_lower.ends_with(".tsx")
        || path_lower.ends_with(".vue")
        || path_lower.ends_with(".svelte")
        || path_lower.ends_with(".css")
        || path_lower.ends_with(".scss")
        || path_lower.ends_with(".sass")
        || path_lower.ends_with(".less")
        || path_lower.ends_with(".html")
        || path_lower.ends_with(".htm")
        || path_lower.ends_with(".razor")
        || path_lower.ends_with(".cshtml")
        || path_lower.ends_with(".py")
        || path_lower.ends_with(".rs")
        || path_lower.ends_with(".go")
        || path_lower.ends_with(".java")
        || path_lower.ends_with(".cpp")
        || path_lower.ends_with(".c")
        || path_lower.ends_with(".h")
        || path_lower.ends_with(".php")
    {
        return FileType::SourceCode;
    }

    FileType::Other
}

/// analyse change hints from diff content with improved semantic detection
fn analyse_change_hints(content: &str, is_new_file: bool) -> Vec<ChangeHint> {
    let mut hints = Vec::new();
    let content_lower = content.to_lowercase();

    if is_new_file {
        hints.push(ChangeHint::NewFeature);
        hints.push(ChangeHint::MajorAddition);
        return hints;
    }

    // analyse structural additions (strong indicators of new features)
    let added_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.starts_with('+'))
        .map(|line| {
            // use Unicode-safe slicing to remove the '+' prefix
            if line.len() > 1 && line.is_char_boundary(1) {
                &line[1..]
            } else if line.len() > 1 {
                // Find the next character boundary
                let mut pos = 1;
                while pos < line.len() && !line.is_char_boundary(pos) {
                    pos += 1;
                }
                &line[pos..]
            } else {
                ""
            }
        })
        .collect();

    let added_content = added_lines.join("\n").to_lowercase();
    let removed_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.starts_with('-'))
        .collect();

    // count significant additions vs modifications
    let additions_count = added_lines.len();
    let removals_count = removed_lines.len();
    let net_additions = additions_count.saturating_sub(removals_count);

    // detect new structures/functions (multi-language patterns)
    // C# patterns
    if added_content.contains("public class ")
        || added_content.contains("class ")
        || added_content.contains("public interface ")
        || added_content.contains("interface ")
        || added_content.contains("public record ")
        || added_content.contains("record ")
    {
        hints.push(ChangeHint::NewStruct);
        hints.push(ChangeHint::NewFeature);
    }

    if added_content.contains("public enum ") || added_content.contains("enum ") {
        hints.push(ChangeHint::NewEnum);
        hints.push(ChangeHint::NewFeature);
    }

    // JavaScript/TypeScript patterns
    if added_content.contains("export class ")
        || added_content.contains("class ")
        || added_content.contains("export interface ")
        || added_content.contains("interface ")
        || added_content.contains("export type ")
        || added_content.contains("type ")
    {
        hints.push(ChangeHint::NewStruct);
        hints.push(ChangeHint::NewFeature);
    }

    // Function patterns (multi-language)
    if added_content.contains("public ")
        || added_content.contains("function ")
        || added_content.contains("const ")
        || added_content.contains("=> ")
        || added_content.contains("def ")
        || added_content.contains("fn ")
        || added_content.contains("pub fn")
    {
        hints.push(ChangeHint::NewFunction);
        if net_additions > 10 {
            hints.push(ChangeHint::NewFeature);
        }
    }

    // Module/namespace patterns
    if added_content.contains("namespace ")
        || added_content.contains("module ")
        || added_content.contains("export ")
        || added_content.contains("mod ")
    {
        hints.push(ChangeHint::NewModule);
        hints.push(ChangeHint::NewFeature);
    }

    // CSS patterns for new styles
    if added_content.contains(".")
        && (added_content.contains("{") || added_content.contains("}"))
        && net_additions > 5
    {
        hints.push(ChangeHint::NewFeature);
    }

    // determine if this is major addition vs minor tweak
    if net_additions > 20 {
        hints.push(ChangeHint::MajorAddition);
        if !hints.contains(&ChangeHint::NewFeature) {
            hints.push(ChangeHint::NewFeature);
        }
    } else if net_additions <= 5
        && !hints.iter().any(|h| {
            matches!(
                h,
                ChangeHint::NewStruct
                    | ChangeHint::NewEnum
                    | ChangeHint::NewFunction
                    | ChangeHint::NewModule
            )
        })
    {
        hints.push(ChangeHint::MinorTweak);
    }

    // bug fix indicators (but not if we're adding major new functionality)
    if (content_lower.contains("fix")
        || content_lower.contains("bug")
        || content_lower.contains("error")
        || content_lower.contains("issue")
        || content_lower.contains("problem")
        || content_lower.contains("crash"))
        && !hints.contains(&ChangeHint::MajorAddition)
    {
        hints.push(ChangeHint::BugFix);
    }

    // error handling
    if added_content.contains("result")
        || added_content.contains("option")
        || added_content.contains("unwrap")
        || added_content.contains("expect")
        || added_content.contains("context")
    {
        hints.push(ChangeHint::ErrorHandling);
    }

    // refactoring indicators (but only if not adding major new code)
    if (content_lower.contains("refactor")
        || content_lower.contains("rename")
        || content_lower.contains("move")
        || content_lower.contains("extract")
        || content_lower.contains("cleanup"))
        && !hints.contains(&ChangeHint::MajorAddition)
    {
        hints.push(ChangeHint::Refactor);
    }

    // performance indicators
    if content_lower.contains("perf")
        || content_lower.contains("performance")
        || content_lower.contains("optimize")
        || content_lower.contains("speed")
        || content_lower.contains("cache")
        || content_lower.contains("async")
    {
        hints.push(ChangeHint::Performance);
    }

    // dependency changes (multi-platform)
    if (content_lower.contains("dependencies")
        || content_lower.contains("packages")
        || content_lower.contains("using ")
        || content_lower.contains("import ")
        || content_lower.contains("require(")
        || content_lower.contains("from "))
        && (content.contains("package.json")
            || content.contains("Cargo.toml")
            || content.contains(".csproj")
            || content.contains("requirements.txt")
            || added_content.contains("using ")
            || added_content.contains("import "))
    {
        hints.push(ChangeHint::Dependencies);
    }

    // documentation changes
    if added_content.contains("///")
        || added_content.contains("/**")
        || (content_lower.contains("doc") && added_content.contains("//"))
    {
        hints.push(ChangeHint::Documentation);
    }

    // if no specific hints found and it's not a minor tweak, assume it's a new feature
    if hints.is_empty() {
        hints.push(ChangeHint::NewFeature);
    }

    hints
}

/// decode line content with appropriate encoding
fn decode_line_content(content: &[u8]) -> String {
    // try to detect encoding and decode
    let (cow, _encoding_used, had_errors) = Encoding::for_label(b"utf-8").unwrap().decode(content);

    if had_errors {
        // fall back to lossy conversion if there were decoding errors
        String::from_utf8_lossy(content).to_string()
    } else {
        cow.to_string()
    }
}
