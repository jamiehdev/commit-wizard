use crate::git::{DiffInfo, FileType};

pub fn determine_forced_commit_type(diff_info: &DiffInfo) -> Option<String> {
    if diff_info.files.is_empty() {
        return None;
    }

    if diff_info
        .files
        .iter()
        .all(|file| matches!(file.file_type, FileType::Documentation))
    {
        return Some("docs".to_string());
    }

    if diff_info.files.iter().all(|file| is_ci_path(&file.path)) {
        return Some("ci".to_string());
    }

    if diff_info
        .files
        .iter()
        .all(|file| matches!(file.file_type, FileType::Build))
    {
        return Some("build".to_string());
    }

    None
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
    use crate::git::ModifiedFile;

    fn make_file(path: &str, file_type: FileType) -> ModifiedFile {
        ModifiedFile {
            path: path.to_string(),
            added_lines: 1,
            removed_lines: 0,
            diff_content: "+change".to_string(),
            file_type,
            change_hints: Vec::new(),
            is_minified: false,
        }
    }

    #[test]
    fn forces_docs_for_doc_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file("README.md", FileType::Documentation)],
            summary: String::new(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("docs".to_string())
        );
    }

    #[test]
    fn forces_ci_for_workflow_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file(
                ".github/workflows/ci.yml",
                FileType::Config,
            )],
            summary: String::new(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("ci".to_string())
        );
    }

    #[test]
    fn forces_build_for_build_only_changes() {
        let diff_info = DiffInfo {
            files: vec![make_file("Cargo.toml", FileType::Build)],
            summary: String::new(),
        };
        assert_eq!(
            determine_forced_commit_type(&diff_info),
            Some("build".to_string())
        );
    }
}
