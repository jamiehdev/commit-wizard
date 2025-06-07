use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;
use std::fs;
use std::path::Path;

/// check if a file should be analysed based on its extension and size
#[allow(dead_code)]
pub fn should_analyse_file(path: &Path, max_size: usize) -> Result<bool> {
    // skip files that don't exist
    if !path.exists() {
        return Ok(false);
    }
    
    // skip directories
    if path.is_dir() {
        return Ok(false);
    }
    
    // check file size
    let metadata = fs::metadata(path)?;
    if metadata.len() > max_size as u64 {
        return Ok(false);
    }
    
    // skip binary files and minified files
    let _file_name = path.file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    
    lazy_static! {
        // regex patterns for files to ignore
        static ref IGNORED_PATTERNS: Regex = Regex::new(
            r"(?i)(\.min\.|\.bundle\.|\.packed\.|\.compiled\.|\.optimised\.)|node_modules|dist|build|vendor"
        ).unwrap();
        
        // common binary file extensions
        static ref BINARY_EXTENSIONS: Regex = Regex::new(
            r"(?i)\.(jpg|jpeg|png|gif|bmp|ico|svg|webp|mp3|mp4|avi|mov|wmv|flv|mkv|woff|woff2|eot|ttf|otf|exe|dll|so|dylib|bin|dat|o|obj|lib|a|class|jar|war|ear|zip|tar|gz|rar|7z)$"
        ).unwrap();
    }
    
    // skip files with ignored patterns in path
    if IGNORED_PATTERNS.is_match(&path.to_string_lossy()) {
        return Ok(false);
    }
    
    // skip binary files
    if let Some(ext) = path.extension() {
        let ext_str = ext.to_string_lossy();
        if BINARY_EXTENSIONS.is_match(&format!(".{}", ext_str)) {
            return Ok(false);
        }
    }
    
    Ok(true)
}

/// truncate a string to a maximum length with ellipsis
#[allow(dead_code)]
pub fn truncate_with_ellipsis(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        text.to_string()
    } else {
        // Use Unicode-safe truncation to avoid panics with emoji characters
        let truncate_at = std::cmp::min(max_length.saturating_sub(3), text.len());
        let mut end_pos = truncate_at;
        
        // Find the nearest character boundary before truncate_at
        while end_pos > 0 && !text.is_char_boundary(end_pos) {
            end_pos -= 1;
        }
        
        format!("{}...", &text[..end_pos])
    }
}

/// identify likely scope of changes from file paths
#[allow(dead_code)]
pub fn identify_scope(file_paths: &[String]) -> Option<String> {
    if file_paths.is_empty() {
        return None;
    }
    
    // common pattern is to have files organised by feature, component or module
    lazy_static! {
        static ref PATH_COMPONENT: Regex = Regex::new(r"(?:/|^)([a-zA-Z0-9_-]+)(?:/|$)").unwrap();
    }
    
    // count occurrences of path components
    let mut component_counts = std::collections::HashMap::new();
    
    for path in file_paths {
        for cap in PATH_COMPONENT.captures_iter(path) {
            if let Some(component) = cap.get(1) {
                let comp = component.as_str();
                // skip common directories that don't indicate a scope
                if !["src", "lib", "app", "test", "tests", "spec", "specs"].contains(&comp) {
                    *component_counts.entry(comp.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    
    // find the most common component
    component_counts
        .into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(component, _)| component)
}

/// convert a snake_case or kebab-case string to lowercase
#[allow(dead_code)]
pub fn format_scope(scope: &str) -> String {
    scope.to_lowercase()
} 