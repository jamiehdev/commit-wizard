use anyhow::{Context, Result};
use git2::{Repository, DiffOptions, DiffFormat};
use encoding_rs::Encoding;

/// information about a modified file in the git diff
pub struct ModifiedFile {
    pub path: String,
    pub added_lines: usize,
    pub removed_lines: usize,
    pub diff_content: String,
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
    verbose: bool
) -> Result<DiffInfo> {
    // open the repository
    let repo = Repository::discover(repo_path)
        .context("failed to open git repository")?;
    
    // create diff options
    let mut diff_opts = DiffOptions::new();
    diff_opts.show_binary(false);
    diff_opts.include_untracked(true);
    diff_opts.recurse_untracked_dirs(true);
    
    let mut files = Vec::new();
    
    // check if repository has any commits
    let has_head = match repo.head() {
        Ok(_) => true,
        Err(_) => false,
    };
    
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
                                println!("skipping large file: {} ({} KB)", path_str, metadata.len() / 1024);
                            }
                            continue;
                        }
                    }
                    
                    // read file content for new files
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        let line_count = content.lines().count();
                        
                        if verbose {
                            println!("adding new file: {} ({} lines)", path_str, line_count);
                        }
                        
                        // add file to the list
                        files.push(ModifiedFile {
                            path: path_str.to_string(),
                            added_lines: line_count,
                            removed_lines: 0,
                            diff_content: format!("+{}", content),
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
    
    // generate file list for summary
    let file_list: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
    summary.push_str("\nmodified files:\n");
    summary.push_str(&file_list.join("\n"));
    
    Ok(DiffInfo { files, summary })
}

/// check if there are any staged changes in the repository
pub fn has_staged_changes(repo_path: &str) -> Result<bool> {
    // open the repository
    let repo = Repository::discover(repo_path)
        .context("failed to open git repository")?;
    
    // check if repository has any commits
    let has_head = match repo.head() {
        Ok(_) => true,
        Err(_) => false,
    };
    
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
    let repo = Repository::discover(repo_path)
        .context("failed to open git repository")?;
    
    let mut staged_files = Vec::new();
    
    // check if repository has any commits
    let has_head = match repo.head() {
        Ok(_) => true,
        Err(_) => false,
    };
    
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
                    None, None, None
                )?;
            }
        }
    }
    
    Ok(staged_files)
}

/// process a diff to extract file information
fn process_diff(
    diff: &git2::Diff,
    files: &mut Vec<ModifiedFile>,
    max_file_size: usize,
    max_files: usize,
    verbose: bool
) -> Result<()> {
    diff.print(DiffFormat::Patch, |delta, _, line| {
        let path = match delta.new_file().path() {
            Some(path) => path.to_string_lossy().to_string(),
            None => return true,
        };
        
        // skip binary files
        if delta.new_file().is_binary() || delta.old_file().is_binary() {
            if verbose {
                println!("skipping binary file: {}", path);
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
        
        // find or create the file entry
        let file_entry = match files.iter_mut().find(|f: &&mut ModifiedFile| f.path == path) {
            Some(entry) => entry,
            None => {
                // skip if we've already reached max_files
                if files.len() >= max_files {
                    return true;
                }
                
                files.push(ModifiedFile {
                    path: path.clone(),
                    added_lines: 0,
                    removed_lines: 0,
                    diff_content: String::new(),
                });
                
                files.last_mut().unwrap()
            }
        };
        
        // track added and removed lines
        match line.origin() {
            '+' => file_entry.added_lines += 1,
            '-' => file_entry.removed_lines += 1,
            _ => {}
        }
        
        // convert line content to UTF-8 string
        let content = decode_line_content(line.content());
        
        // append line to diff content (up to a reasonable size)
        if file_entry.diff_content.len() < 5000 {
            file_entry.diff_content.push(line.origin());
            file_entry.diff_content.push_str(&content);
        }
        
        true
    })?;
    
    Ok(())
}

/// decode line content with appropriate encoding
fn decode_line_content(content: &[u8]) -> String {
    // try to detect encoding and decode
    let (cow, _encoding_used, had_errors) = Encoding::for_label(b"utf-8")
        .unwrap()
        .decode(content);
    
    if had_errors {
        // fall back to lossy conversion if there were decoding errors
        String::from_utf8_lossy(content).to_string()
    } else {
        cow.to_string()
    }
}
