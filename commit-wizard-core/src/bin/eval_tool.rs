use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use serde::{Deserialize, Serialize};

use commit_wizard_core::ai::analyse_commit_intelligence;
use commit_wizard_core::analysis::{derive_scope_candidates, determine_forced_commit_type};
use commit_wizard_core::git::{analyse_change_hints, classify_file_type, is_minified_file};
use commit_wizard_core::{
    generate_conventional_commit_with_model, Config, DiffInfo, ModifiedFile,
};

#[derive(Parser, Debug)]
#[command(name = "commit-wizard-eval")]
struct EvalArgs {
    /// Path to eval fixtures directory.
    #[arg(long, default_value = "fixtures/eval")]
    fixtures: String,

    /// Use AI to generate commit messages.
    #[arg(long)]
    use_ai: bool,

    /// Use structured classifier output (requires --use-ai).
    #[arg(long)]
    use_classifier: bool,

    /// Override model for AI evaluation.
    #[arg(long)]
    model: Option<String>,

    /// Output JSON for automation.
    #[arg(long)]
    headless_json: bool,

    /// Limit number of fixtures processed.
    #[arg(long)]
    max_cases: Option<usize>,

    /// Minimum required type accuracy (0.0-1.0).
    #[arg(long)]
    min_type_accuracy: Option<f32>,

    /// Minimum required scope accuracy (0.0-1.0).
    #[arg(long)]
    min_scope_accuracy: Option<f32>,

    /// Minimum required subject accuracy (0.0-1.0).
    #[arg(long)]
    min_subject_accuracy: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct EvalFixture {
    id: String,
    #[serde(default)]
    description: Option<String>,
    files: Vec<EvalFile>,
    #[serde(default)]
    expected: Option<ExpectedCommit>,
}

#[derive(Debug, Deserialize)]
struct EvalFile {
    path: String,
    diff: String,
}

#[derive(Debug, Deserialize)]
struct ExpectedCommit {
    #[serde(rename = "type")]
    commit_type: Option<String>,
    scope: Option<String>,
    subject: Option<String>,
}

#[derive(Debug, Serialize)]
struct EvalReport {
    total: usize,
    ai_used: bool,
    cases: Vec<EvalCaseResult>,
    metrics: Option<EvalMetrics>,
}

#[derive(Debug, Serialize)]
struct EvalMetrics {
    type_accuracy: Option<f32>,
    scope_accuracy: Option<f32>,
    subject_accuracy: Option<f32>,
}

#[derive(Debug, Serialize)]
struct EvalCaseResult {
    id: String,
    status: String,
    description: Option<String>,
    expected_type: Option<String>,
    expected_scope: Option<String>,
    expected_subject: Option<String>,
    predicted_type: Option<String>,
    predicted_scope: Option<String>,
    predicted_subject: Option<String>,
    scope_candidates: Vec<String>,
    commit_message: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = EvalArgs::parse();

    let fixtures_path = PathBuf::from(&args.fixtures);
    if !fixtures_path.exists() {
        return Err(anyhow::anyhow!(
            "fixtures directory not found: {}",
            fixtures_path.display()
        ));
    }

    let mut fixtures = load_fixtures(&fixtures_path)?;
    if let Some(max_cases) = args.max_cases {
        fixtures.truncate(max_cases);
    }

    let config = Config::default();

    if args.use_classifier && !args.use_ai {
        return Err(anyhow::anyhow!(
            "--use-classifier requires --use-ai for evaluation"
        ));
    }

    let mut cases = Vec::new();
    for fixture in fixtures {
        cases.push(run_case(&fixture, &args, &config).await?);
    }

    let metrics = compute_metrics(&cases);
    enforce_thresholds(&metrics, &args)?;
    let report = EvalReport {
        total: cases.len(),
        ai_used: args.use_ai,
        cases,
        metrics,
    };

    if args.headless_json {
        println!("{}", serde_json::to_string(&report)?);
    } else {
        print_human_report(&report);
    }

    Ok(())
}

fn load_fixtures(path: &Path) -> Result<Vec<EvalFixture>> {
    let mut fixtures = Vec::new();

    for entry in fs::read_dir(path).context("failed to read fixtures directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read fixture {}", path.display()))?;

        if content.trim().starts_with('[') {
            let mut items: Vec<EvalFixture> = serde_json::from_str(&content)
                .with_context(|| format!("invalid fixture array {}", path.display()))?;
            fixtures.append(&mut items);
        } else {
            let fixture: EvalFixture = serde_json::from_str(&content)
                .with_context(|| format!("invalid fixture {}", path.display()))?;
            fixtures.push(fixture);
        }
    }

    Ok(fixtures)
}

async fn run_case(
    fixture: &EvalFixture,
    args: &EvalArgs,
    config: &Config,
) -> Result<EvalCaseResult> {
    let diff_info = build_diff_info(fixture)?;
    let intelligence = analyse_commit_intelligence(&diff_info);
    let scope_candidates = derive_scope_candidates(&diff_info);

    let forced_type = determine_forced_commit_type(&diff_info);
    let (status, predicted_type, predicted_scope, predicted_subject, commit_message) =
        if args.use_ai {
            ensure_api_key_present()?;
            let result = generate_conventional_commit_with_model(
                &diff_info,
                false,
                false,
                args.model.clone(),
                config,
                true,
                args.use_classifier,
                None,
                None,
                commit_wizard_core::analysis::ScopeStrategy::Auto,
                true,
                commit_wizard_core::analysis::AiControl::Balanced,
            )
            .await?;
            let generated = result.message;

            let (commit_type, scope, subject) = parse_commit_header(&generated);
            (
                "ok".to_string(),
                commit_type,
                scope,
                subject,
                Some(generated),
            )
        } else {
            (
                "ai_skipped".to_string(),
                forced_type
                    .clone()
                    .or_else(|| Some(intelligence.commit_type_hint.clone())),
                if forced_type.is_some() {
                    None
                } else {
                    intelligence.scope_hint.clone()
                },
                None,
                None,
            )
        };

    Ok(EvalCaseResult {
        id: fixture.id.clone(),
        status,
        description: fixture.description.clone(),
        expected_type: fixture
            .expected
            .as_ref()
            .and_then(|expected| expected.commit_type.clone()),
        expected_scope: fixture
            .expected
            .as_ref()
            .and_then(|expected| expected.scope.clone()),
        expected_subject: fixture
            .expected
            .as_ref()
            .and_then(|expected| expected.subject.clone()),
        predicted_type,
        predicted_scope,
        predicted_subject,
        scope_candidates,
        commit_message,
    })
}

fn build_diff_info(fixture: &EvalFixture) -> Result<DiffInfo> {
    let mut files = Vec::new();
    let mut total_added = 0usize;
    let mut total_removed = 0usize;

    for file in &fixture.files {
        let (added_lines, removed_lines) = count_diff_lines(&file.diff);
        total_added += added_lines;
        total_removed += removed_lines;

        let file_type = classify_file_type(&file.path);
        let is_minified = is_minified_file(&file.path, Some(&file.diff));
        let change_hints = if is_minified {
            Vec::new()
        } else {
            analyse_change_hints(&file.diff, false)
        };

        files.push(ModifiedFile {
            path: file.path.clone(),
            added_lines,
            removed_lines,
            diff_content: file.diff.clone(),
            file_type,
            change_hints,
            is_minified,
        });
    }

    let summary = format!(
        "{} file{} changed, {} insertion{}, {} deletion{}",
        files.len(),
        if files.len() == 1 { "" } else { "s" },
        total_added,
        if total_added == 1 { "" } else { "s" },
        total_removed,
        if total_removed == 1 { "" } else { "s" },
    );

    Ok(DiffInfo {
        files,
        summary,
        repo_root: ".".to_string(),
    })
}

fn count_diff_lines(diff: &str) -> (usize, usize) {
    let mut added = 0usize;
    let mut removed = 0usize;

    for line in diff.lines() {
        if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("diff --git")
        {
            continue;
        }
        if line.starts_with('+') {
            added += 1;
        } else if line.starts_with('-') {
            removed += 1;
        }
    }

    (added, removed)
}

fn parse_commit_header(message: &str) -> (Option<String>, Option<String>, Option<String>) {
    let first_line = message.lines().next().unwrap_or("").trim();
    if first_line.is_empty() {
        return (None, None, None);
    }

    let mut type_part = first_line;
    let mut subject_part = "";
    if let Some(colon_pos) = first_line.find(':') {
        type_part = first_line[..colon_pos].trim();
        subject_part = first_line[colon_pos + 1..].trim();
    }

    let mut commit_type = type_part.trim_end_matches('!').to_string();
    let mut scope = None;

    if let Some(open_paren) = commit_type.find('(') {
        if let Some(close_paren) = commit_type.find(')') {
            if close_paren > open_paren {
                let scope_value = commit_type[open_paren + 1..close_paren].trim();
                if !scope_value.is_empty() {
                    scope = Some(scope_value.to_string());
                }
                commit_type = commit_type[..open_paren].to_string();
            }
        }
    }

    let commit_type = if commit_type.is_empty() {
        None
    } else {
        Some(commit_type)
    };
    let subject = if subject_part.is_empty() {
        None
    } else {
        Some(subject_part.to_string())
    };

    (commit_type, scope, subject)
}

fn compute_metrics(cases: &[EvalCaseResult]) -> Option<EvalMetrics> {
    let mut type_total = 0usize;
    let mut type_correct = 0usize;
    let mut scope_total = 0usize;
    let mut scope_correct = 0usize;
    let mut subject_total = 0usize;
    let mut subject_correct = 0usize;

    for case in cases {
        if let Some(expected_type) = case.expected_type.as_ref() {
            type_total += 1;
            if case.predicted_type.as_ref() == Some(expected_type) {
                type_correct += 1;
            }
        }

        if let Some(expected_scope) = case.expected_scope.as_ref() {
            scope_total += 1;
            if case.predicted_scope.as_ref() == Some(expected_scope) {
                scope_correct += 1;
            }
        }

        if let Some(expected_subject) = case.expected_subject.as_ref() {
            subject_total += 1;
            if case.predicted_subject.as_ref() == Some(expected_subject) {
                subject_correct += 1;
            }
        }
    }

    if type_total == 0 && scope_total == 0 && subject_total == 0 {
        return None;
    }

    Some(EvalMetrics {
        type_accuracy: if type_total > 0 {
            Some(type_correct as f32 / type_total as f32)
        } else {
            None
        },
        scope_accuracy: if scope_total > 0 {
            Some(scope_correct as f32 / scope_total as f32)
        } else {
            None
        },
        subject_accuracy: if subject_total > 0 {
            Some(subject_correct as f32 / subject_total as f32)
        } else {
            None
        },
    })
}

fn print_human_report(report: &EvalReport) {
    println!("commit-wizard eval report");
    println!("cases: {}", report.total);
    println!("ai used: {}", report.ai_used);

    if let Some(metrics) = &report.metrics {
        if let Some(value) = metrics.type_accuracy {
            println!("type accuracy: {:.1}%", value * 100.0);
        }
        if let Some(value) = metrics.scope_accuracy {
            println!("scope accuracy: {:.1}%", value * 100.0);
        }
        if let Some(value) = metrics.subject_accuracy {
            println!("subject accuracy: {:.1}%", value * 100.0);
        }
    }

    for case in &report.cases {
        println!("- {} ({})", case.id, case.status);
        if let Some(description) = &case.description {
            println!("  description: {description}");
        }
        if let Some(message) = &case.commit_message {
            println!("  message: {}", message.lines().next().unwrap_or(""));
        }
    }
}

fn ensure_api_key_present() -> Result<()> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .context("OPENROUTER_API_KEY environment variable is not set")?;
    if api_key.trim().is_empty() {
        return Err(anyhow::anyhow!("OPENROUTER_API_KEY is empty"));
    }
    Ok(())
}

fn enforce_thresholds(metrics: &Option<EvalMetrics>, args: &EvalArgs) -> Result<()> {
    if metrics.is_none() {
        return Ok(());
    }
    let metrics = metrics.as_ref().unwrap();

    if let (Some(min), Some(actual)) = (args.min_type_accuracy, metrics.type_accuracy) {
        if actual < min {
            return Err(anyhow::anyhow!(
                "type accuracy {:.2} below required {:.2}",
                actual,
                min
            ));
        }
    }

    if let (Some(min), Some(actual)) = (args.min_scope_accuracy, metrics.scope_accuracy) {
        if actual < min {
            return Err(anyhow::anyhow!(
                "scope accuracy {:.2} below required {:.2}",
                actual,
                min
            ));
        }
    }

    if let (Some(min), Some(actual)) = (args.min_subject_accuracy, metrics.subject_accuracy) {
        if actual < min {
            return Err(anyhow::anyhow!(
                "subject accuracy {:.2} below required {:.2}",
                actual,
                min
            ));
        }
    }

    Ok(())
}
