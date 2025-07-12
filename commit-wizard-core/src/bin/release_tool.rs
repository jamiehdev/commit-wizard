// new release tool binary for automated versioning and changelog generation
use anyhow::{Context, Result};
use git2::{Commit, Repository};
use regex::Regex;
use semver::{Prerelease, Version};
use std::env;
use std::fs;
use std::path::Path;
use std::process::{exit, Command};

/// entrypoint – parse args then run
fn main() -> Result<()> {
    // obey the user's lowercase comment preference
    let args: Vec<String> = env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let force_major = args.iter().any(|a| a == "--major");

    let repo = Repository::open(".").context("cannot open git repository")?;

    let current_version = read_workspace_version()?;
    let commits = commits_since_last_tag(&repo)?;
    let analysis = classify_commits(&commits);

    // compute next version according to semver and conventional commits
    let branch = current_branch(&repo)?;
    let mut next_version = current_version.clone();
    
    // if on a release branch and current version is prerelease, create stable version
    if branch.starts_with("release/") && !current_version.pre.is_empty() {
        next_version.pre = Prerelease::EMPTY;
    } else {
        // normal version bumping logic
        if force_major || analysis.breaking {
            next_version.major += 1;
            next_version.minor = 0;
            next_version.patch = 0;
        } else if analysis.has_feat {
            next_version.minor += 1;
            next_version.patch = 0;
        } else if analysis.has_fix {
            next_version.patch += 1;
        } else {
            println!("no releasable changes detected");
            exit(0);
        }

        // add prerelease metadata if not on main or release/*
        if branch != "main" && !branch.starts_with("release/") {
            next_version.pre = Prerelease::new("beta")?;
        }
    }

    if dry_run {
        println!("would release {next_version}");
        exit(0);
    }

    // update versions across the repo and write changelog
    update_versions(&next_version)?;
    prepend_changelog(&commits, &next_version)?;

    // commit and tag
    commit_and_tag(&repo, &next_version)?;

    println!("released {next_version}");
    Ok(())
}

/// helper holding commit analysis
struct CommitAnalysis {
    has_feat: bool,
    has_fix: bool,
    breaking: bool,
}

/// scan commit messages since last tag and classify
fn classify_commits(commits: &[Commit]) -> CommitAnalysis {
    let mut has_feat = false;
    let mut has_fix = false;
    let mut breaking = false;
    for commit in commits {
        let msg = commit.summary().unwrap_or("");
        if msg.starts_with("feat") {
            has_feat = true;
        }
        if msg.starts_with("fix") || msg.starts_with("perf") || msg.starts_with("refactor") {
            has_fix = true; // treat as patch
        }
        if msg.contains("BREAKING CHANGE") || msg.contains("!") {
            breaking = true;
        }
    }
    CommitAnalysis {
        has_feat,
        has_fix,
        breaking,
    }
}

/// get commits since last semver tag (vX.Y.Z)
fn commits_since_last_tag(repo: &Repository) -> Result<Vec<Commit>> {
    let mut list = Vec::new();
    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;

    let tag_re = Regex::new(r"^v\d+\.\d+\.\d+$").unwrap();
    for id in revwalk {
        let id = id?;
        let commit = repo.find_commit(id)?;
        // stop if commit has a semver tag
        let tags = repo.tag_names(None)?;
        for name in tags.iter().flatten() {
            if let Ok(obj) = repo.revparse_single(name) {
                if obj.id() == commit.id() && tag_re.is_match(name) {
                    return Ok(list);
                }
            }
        }
        list.push(commit);
    }
    Ok(list)
}

/// read the workspace version from the root cargo.toml
fn read_workspace_version() -> Result<Version> {
    let content = fs::read_to_string("Cargo.toml")?;
    let re = Regex::new(r#"version\s*=\s*"([^"]+)""#).unwrap();
    let caps = re
        .captures(&content)
        .context("cannot find version in Cargo.toml")?;
    Version::parse(&caps[1]).map_err(Into::into)
}

/// update versions in cargo workspace and node packages
fn update_versions(ver: &Version) -> Result<()> {
    write_cargo_version(ver)?;
    
    // use the sync script to propagate version to all package.json files
    run_version_sync_script()?;
    
    Ok(())
}

/// run the version sync script to propagate version from cargo.toml to package.json files
fn run_version_sync_script() -> Result<()> {
    let output = Command::new("node")
        .arg("commit-wizard-napi/scripts/sync-version.mjs")
        .output()
        .context("failed to run version sync script")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "version sync script failed: {}", stderr
        ));
    }
    
    // print the script output for visibility
    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("{}", stdout);
    
    Ok(())
}

fn write_cargo_version(ver: &Version) -> Result<()> {
    let cargo_toml_path = "Cargo.toml";
    let content = fs::read_to_string(cargo_toml_path)?;
    let re = Regex::new(r#"version\s*=\s*"([^"]+)""#).unwrap();
    let new_content = re.replace(&content, format!("version = \"{}\"", ver));
    fs::write(cargo_toml_path, new_content.as_bytes())?;
    Ok(())
}



/// prepend entries to changelog file
fn prepend_changelog(commits: &[Commit], ver: &Version) -> Result<()> {
    let mut section = format!("## v{}\n\n", ver);
    for commit in commits {
        section.push_str(&format!("- {}\n", commit.summary().unwrap_or("")));
    }
    section.push('\n');

    let existing = fs::read_to_string("CHANGELOG.md").unwrap_or_default();
    fs::write("CHANGELOG.md", format!("{}{}", section, existing))?;
    Ok(())
}

/// determine current branch name
fn current_branch(repo: &Repository) -> Result<String> {
    Ok(repo.head()?.shorthand().unwrap_or("head").to_string())
}

/// commit updated files and create tag
fn commit_and_tag(repo: &Repository, ver: &Version) -> Result<()> {
    // stage all modified files
    let mut index = repo.index()?;
    index.add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;

    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let sig = repo.signature()?;
    let head_commit = repo.head()?.peel_to_commit()?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &format!("chore(release): v{}", ver),
        &tree,
        &[&head_commit],
    )?;

    // create tag
    let obj = repo.revparse_single("HEAD")?;
    repo.tag(
        &format!("v{}", ver),
        &obj,
        &sig,
        &format!("v{}", ver),
        false,
    )?;

    Ok(())
}
