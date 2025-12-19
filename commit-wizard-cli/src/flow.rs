use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use anyhow::{Context, Result};
use console::style;
use dialoguer::{theme::ColorfulTheme, Select};
use indicatif::{ProgressBar, ProgressStyle};

use commit_wizard_core::{
    ai, commit_with_message, ensure_api_key_present, fetch_openrouter_models,
    generate_commit_message, get_current_model, get_model_description, load_config,
    run_headless_flow, save_config, test_git_diff_processing, Config, CoreCliArgs,
};

pub async fn run_cli(args: CoreCliArgs) -> Result<(String, bool)> {
    validate_args(&args)?;

    let mut config = load_config()?;

    if args.test_diff {
        return test_git_diff_processing(&args).await;
    }

    let is_headless = args.headless || args.headless_json;
    if is_headless {
        ensure_api_key_present()?;
        return run_headless_flow(args, &mut config).await;
    }

    ensure_api_key_present()?;

    // background refresh of model catalogue (non-blocking)
    let is_updating_models = Arc::new(AtomicBool::new(true));
    let just_updated_models = Arc::new(AtomicBool::new(false));
    {
        let updating = is_updating_models.clone();
        let updated = just_updated_models.clone();
        tokio::spawn(async move {
            // refresh model cache; ignore errors silently
            let _ = fetch_openrouter_models().await;
            updating.store(false, Ordering::Relaxed);
            updated.store(true, Ordering::Relaxed);
        });
    }

    // welcome banner
    println!("{}", style("\ncommit-wizard 🧙").cyan().bold());
    println!(
        "{}\n",
        style("ai-powered conventional commit message generator").dim()
    );

    loop {
        // show one-time background update completion notice
        if just_updated_models.swap(false, Ordering::Relaxed) {
            println!(
                "{}",
                style("✅ model catalogue updated in the background").green()
            );
        }

        let current_model_text = if config.auto_select {
            "auto-complexity".to_string()
        } else {
            get_current_model(&config, &args, None)
        };

        let model_settings_option = {
            let updating_suffix = if is_updating_models.load(Ordering::Relaxed) {
                style(" (updating...)").dim().to_string()
            } else {
                String::new()
            };
            format!(
                "model settings - current: {}{}",
                style(&current_model_text).yellow(),
                updating_suffix
            )
        };

        let options = &[
            "generate commit message".to_string(),
            model_settings_option,
            "refresh model catalogue now".to_string(),
            "exit".to_string(),
        ];

        let selection = Select::with_theme(&ColorfulTheme::default())
            .with_prompt("what would you like to do?")
            .default(0)
            .items(options)
            .interact()?;

        match selection {
            0 => {
                // generate commit message
                return run_generate_and_commit_flow(args.clone(), &mut config).await;
            }
            1 => {
                // model settings
                handle_model_settings(&mut config, &args).await?;
                println!();
            }
            2 => {
                // manual model refresh
                let pb = ProgressBar::new_spinner();
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
                        .template("{spinner:.blue} updating model catalogue...")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                pb.enable_steady_tick(Duration::from_millis(80));
                match fetch_openrouter_models().await {
                    Ok(models) => {
                        pb.finish_and_clear();
                        println!(
                            "{} {} {}",
                            style("✅").green(),
                            style(models.len()).yellow().bold(),
                            style("models available (catalogue refreshed)").green()
                        );
                    }
                    Err(e) => {
                        pb.finish_and_clear();
                        eprintln!("{} {}", style("⚠️  failed to refresh models:").yellow(), e);
                    }
                }
                println!();
            }
            3 => {
                println!("\n{}", style("👋 bye!").dim());
                return Ok(("".to_string(), false));
            }
            _ => unreachable!(),
        }
    }
}

fn validate_args(args: &CoreCliArgs) -> Result<()> {
    if args.headless_json && args.debug {
        return Err(anyhow::anyhow!(
            "--debug is not supported with --headless-json (use --headless instead)"
        ));
    }
    if let Some(min_confidence) = args.min_confidence {
        if !(0.0..=1.0).contains(&min_confidence) {
            return Err(anyhow::anyhow!(
                "--min-confidence must be between 0.0 and 1.0"
            ));
        }
        if !args.use_classifier {
            return Err(anyhow::anyhow!(
                "--min-confidence requires --use-classifier"
            ));
        }
    }
    if let Some(min_scope_confidence) = args.min_scope_confidence {
        if !(0.0..=1.0).contains(&min_scope_confidence) {
            return Err(anyhow::anyhow!(
                "--min-scope-confidence must be between 0.0 and 1.0"
            ));
        }
        if !args.use_classifier {
            return Err(anyhow::anyhow!(
                "--min-scope-confidence requires --use-classifier"
            ));
        }
    }
    Ok(())
}

async fn run_generate_and_commit_flow(
    args: CoreCliArgs,
    config: &mut Config,
) -> Result<(String, bool)> {
    let is_headless = args.headless || args.headless_json;
    let repo_path = args.path.clone().unwrap_or_else(|| ".".to_string());

    if args.smart_model {
        println!("{}", style("🤖 smart model selection enabled").green());
        println!(
            "{}\n",
            style("automatically choosing optimal model based on commit complexity").dim()
        );
    }

    match commit_wizard_core::git::has_staged_changes(&repo_path) {
        Ok(has_staged) => {
            if has_staged {
                if let Ok(files) = commit_wizard_core::git::get_staged_files(&repo_path) {
                    println!("{}\n", style("staged files:").cyan().bold());
                    for file in files {
                        println!("{}", style(format!("  - {file}")).green());
                    }
                    println!();
                }
            } else {
                println!(
                    "{}\n",
                    style("⚠️  no staged changes found, will analyse unstaged changes instead")
                        .yellow()
                        .bold()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "{} {}",
                style("❌ error checking staged changes:").red().bold(),
                style(e).red()
            );
        }
    }

    let generation = generate_commit_message(
        &args,
        config,
        is_headless,
        args.use_classifier,
    )
    .await?;

    let diff_info = generation.diff_info;
    let mut commit_message = generation.commit_message;
    let mut selected_model = generation.selected_model;
    let used_fallback = generation.used_fallback;

    if args.verbose {
        println!("found {} modified files", diff_info.files.len());
        for file in &diff_info.files {
            println!(
                "- {} ({} lines added, {} lines removed)",
                file.path, file.added_lines, file.removed_lines
            );
        }
    }

    println!("{}", style("🤖 selected model:").cyan().bold());
    if config.auto_select {
        println!(
            "{} {}",
            style(&get_model_description(config, &selected_model)).yellow(),
            style("(auto-complexity)").dim()
        );
    } else {
        println!("{}", style(&get_model_description(config, &selected_model)).yellow());
    }
    if used_fallback {
        println!(
            "{} {}",
            style("✅ recovered with model:").green(),
            style(&get_model_description(config, &selected_model)).yellow()
        );
    }
    println!();

    println!(
        "\n{}\n",
        style("✅ generated commit message:").green().bold()
    );
    println!("{}", style(&commit_message).yellow());
    println!();

    let mut should_commit_now = args.yes;
    let mut commit_succeeded = false;

    if !args.yes {
        println!("{}", style("press ctrl+c at any time to exit").dim());

        loop {
            let options = &[
                "yes, commit this message",
                "edit this message",
                "no, regenerate message",
                "model settings",
            ];
            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("what would you like to do?")
                .default(0)
                .items(options)
                .interact()?;

            match selection {
                0 => {
                    println!("{}", style("proceeding with commit...").green());
                    should_commit_now = true;
                    break;
                }
                1 => {
                    println!("{}", style("opening editor for commit message...").cyan());
                    if let Some(edited_message) = open_editor_for_message(&commit_message)? {
                        commit_message = edited_message;
                        println!("{}", style("commit message updated").green());
                    } else {
                        println!(
                            "{}",
                            style("edit cancelled, using previous message").yellow()
                        );
                    }
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                2 => {
                    println!("\n{}", style("regenerating...").cyan());
                    commit_message = ai::generate_conventional_commit_with_model(
                        &diff_info,
                        args.debug,
                        args.smart_model,
                        Some(selected_model.clone()),
                        config,
                        is_headless,
                        args.use_classifier,
                        args.min_confidence,
                        args.min_scope_confidence,
                        args.scope_strategy,
                        args.scope_aliases,
                        args.ai_control,
                    )
                    .await
                    .context("failed to regenerate commit message")?
                    .message;
                    println!(
                        "\n{}\n",
                        style("✅ newly generated commit message:").green().bold()
                    );
                    println!("{}", style(&commit_message).yellow());
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                3 => {
                    println!("\n{}", style("model settings").cyan().bold());

                    match select_model_interactively(config).await {
                        Ok(new_model) => {
                            if new_model == "AUTO_COMPLEXITY" {
                                config.auto_select = true;
                                config.current_model = None;
                                if let Err(e) = save_config(config) {
                                    eprintln!(
                                        "{} {}",
                                        style(
                                            "⚠️  warning: failed to save auto-complexity setting:"
                                        )
                                        .yellow(),
                                        e
                                    );
                                } else {
                                    println!(
                                        "{}",
                                        style("✅ auto-complexity selection enabled and saved")
                                            .green()
                                    );
                                }

                                selected_model =
                                    get_current_model(config, &args, Some(&diff_info));
                                println!(
                                    "{} {}",
                                    style("🤖 auto-selected:").cyan(),
                                    style(get_model_description(config, &selected_model))
                                        .yellow()
                                );
                            } else {
                                config.current_model = Some(new_model.clone());
                                config.auto_select = false;
                                if let Err(e) = save_config(config) {
                                    eprintln!(
                                        "{} {}",
                                        style("⚠️  warning: failed to save model preference:")
                                            .yellow(),
                                        e
                                    );
                                } else {
                                    println!(
                                        "{} {}",
                                        style("✅ model preference saved:").green(),
                                        style(get_model_description(config, &new_model)).yellow()
                                    );
                                }

                                selected_model = new_model;
                            }

                            // regenerate with new model
                            println!("\n{}", style("regenerating with new model...").cyan());
                            commit_message = ai::generate_conventional_commit_with_model(
                                &diff_info,
                                args.debug,
                                args.smart_model,
                                Some(selected_model.clone()),
                                config,
                                is_headless,
                                args.use_classifier,
                                args.min_confidence,
                                args.min_scope_confidence,
                                args.scope_strategy,
                                args.scope_aliases,
                                args.ai_control,
                            )
                            .await
                            .context("failed to regenerate commit message with new model")?
                            .message;
                            println!(
                                "\n{}\n",
                                style("✅ newly generated commit message:").green().bold()
                            );
                            println!("{}", style(&commit_message).yellow());
                        }
                        Err(e) => {
                            if e.to_string() != "cancelled" {
                                eprintln!(
                                    "{} {}",
                                    style("⚠️  model selection failed:").yellow(),
                                    e
                                );
                            }
                        }
                    }
                    println!("\n{}", style("current commit message:").cyan().bold());
                    println!("{}", style(&commit_message).yellow());
                    println!();
                }
                _ => unreachable!(),
            }
        }
    } else {
        println!(
            "{}",
            style("--yes flag detected, proceeding with generated message automatically.")
                .green()
        );
    }

    if should_commit_now {
        println!("{}", style("executing commit command...").cyan());
        let outcome = commit_with_message(&repo_path, &commit_message)?;
        if outcome.success {
            println!("{}", style("\n✅ commit successful!").green().bold());
            if !outcome.stdout.trim().is_empty() {
                println!("{}", outcome.stdout.trim());
            }
            commit_succeeded = true;
        } else {
            eprintln!("{}", style("\n❌ commit failed:").red().bold());
            if !outcome.stderr.trim().is_empty() {
                eprintln!("{}", outcome.stderr.trim());
            }
            return Err(anyhow::anyhow!("git commit command failed"));
        }
    }

    Ok((commit_message, commit_succeeded))
}

async fn handle_model_settings(config: &mut Config, args: &CoreCliArgs) -> Result<()> {
    let current_model_desc = get_model_description(config, &get_current_model(config, args, None));

    println!("\n{}", style("model settings").cyan().bold());
    println!(
        "{} {}",
        style("current model:").dim(),
        style(current_model_desc).yellow()
    );
    println!();

    match select_model_interactively(config).await {
        Ok(new_model) => {
            if new_model == "AUTO_COMPLEXITY" {
                config.auto_select = true;
                config.current_model = None;
                if let Err(e) = save_config(config) {
                    eprintln!(
                        "{} {}",
                        style("⚠️  warning: failed to save auto-complexity setting:").yellow(),
                        e
                    );
                } else {
                    println!(
                        "{}",
                        style("✅ auto-complexity selection enabled and saved").green()
                    );
                }
            } else {
                config.current_model = Some(new_model.clone());
                config.auto_select = false;
                if let Err(e) = save_config(config) {
                    eprintln!(
                        "{} {}",
                        style("⚠️  warning: failed to save model preference:").yellow(),
                        e
                    );
                } else {
                    let new_model_desc = get_model_description(config, &new_model);
                    println!(
                        "{} {}",
                        style("✅ model updated:").green(),
                        style(new_model_desc).yellow()
                    );
                }
            }
        }
        Err(e) => {
            if e.to_string() != "cancelled" {
                eprintln!("{} {}", style("⚠️  model selection failed:").yellow(), e);
            }
        }
    }
    Ok(())
}

fn open_editor_for_message(current_message: &str) -> Result<Option<String>> {
    use crossterm::terminal::disable_raw_mode;
    use std::{
        env,
        fs::{self, File},
        io::Write,
        process::{Command, Stdio},
        time::{SystemTime, UNIX_EPOCH},
    };
    use which::which;

    let millis = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    let tmp_path = env::temp_dir().join(format!("commit-wizard-{millis}.txt"));

    {
        let mut file = File::create(&tmp_path)
            .with_context(|| format!("failed to create {}", tmp_path.display()))?;
        file.write_all(current_message.as_bytes())
            .context("failed to write initial commit message")?;
    }

    let _ = disable_raw_mode();

    let editor = if let Ok(vis) = env::var("VISUAL") {
        vis
    } else if let Ok(ed) = env::var("EDITOR") {
        ed
    } else {
        let candidates = ["vim", "vi", "nvim", "code -w", "nano"];

        let mut selected_editor = None;
        for &candidate in &candidates {
            let command = candidate.split_whitespace().next().unwrap();
            if which(command).is_ok() {
                selected_editor = Some(candidate.to_string());
                break;
            }
        }

        match selected_editor {
            Some(editor) => editor,
            None => {
                println!(
                    "{}",
                    style("no suitable editor found, falling back to nano").yellow()
                );
                "nano".to_string()
            }
        }
    };

    let mut editor_parts = editor.split_whitespace();
    let editor_executable = editor_parts.next().unwrap_or(&editor);
    let editor_args = editor_parts.collect::<Vec<&str>>();

    let status = Command::new(editor_executable)
        .args(&editor_args)
        .arg(&tmp_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to execute editor '{editor}'"))?;

    if !status.success() {
        eprintln!(
            "{}",
            style(format!("editor '{editor}' exited with error: {status}")).yellow()
        );
        let _ = fs::remove_file(&tmp_path);
        return Ok(None);
    }

    let edited = fs::read_to_string(&tmp_path)
        .with_context(|| format!("failed to read {}", tmp_path.display()))?;
    let _ = fs::remove_file(&tmp_path);

    if edited.trim_end() != current_message.trim_end() {
        Ok(Some(edited.trim_end().to_string()))
    } else {
        println!(
            "{}",
            style("no changes detected; using previous message").yellow()
        );
        Ok(None)
    }
}

async fn select_model_interactively(config: &Config) -> Result<String> {
    println!("{}", style("🤖 model selection").cyan().bold());

    let main_options = vec![
        "select from configured models",
        "browse all openrouter models",
        "enable auto-complexity selection",
    ];

    let main_choice = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("choose an option")
        .items(&main_options)
        .interact()
        .context("failed to interact with main model menu")?;

    match main_choice {
        0 => {
            let models = ai::get_available_models(config);
            if models.is_empty() {
                return Err(anyhow::anyhow!("no models configured"));
            }

            let model_descriptions: Vec<&str> = models.iter().map(|(_, desc)| *desc).collect();

            let selection = Select::with_theme(&ColorfulTheme::default())
                .with_prompt("choose model")
                .items(&model_descriptions)
                .interact()
                .context("failed to select model")?;

            Ok(models[selection].0.to_string())
        }
        1 => browse_openrouter_models().await,
        2 => {
            println!("{}", style("✅ auto-complexity selection enabled").green());
            Ok("AUTO_COMPLEXITY".to_string())
        }
        _ => unreachable!(),
    }
}

async fn browse_openrouter_models() -> Result<String> {
    println!("{}", style("🔄 fetching models from openrouter...").cyan());

    let all_models = match fetch_openrouter_models().await {
        Ok(models) => models,
        Err(e) => {
            eprintln!("{} {}", style("⚠️  failed to fetch models:").yellow(), e);
            println!("{}", style("falling back to configured models").dim());
            return Err(anyhow::anyhow!("failed to fetch openrouter models"));
        }
    };

    if all_models.is_empty() {
        return Err(anyhow::anyhow!("no models available from openrouter"));
    }

    println!(
        "{} {} {}",
        style("✅").green(),
        style(all_models.len()).yellow().bold(),
        style("models found").green()
    );

    intelligent_model_search(&all_models).await
}

async fn intelligent_model_search(all_models: &[commit_wizard_core::AvailableModel]) -> Result<String> {
    use crossterm::{
        cursor,
        event::{self, Event, KeyCode, KeyEvent},
        execute,
        terminal::{self, disable_raw_mode, enable_raw_mode, size},
    };
    use std::io::{stdout, Write};

    struct CursorGuard;
    impl Drop for CursorGuard {
        fn drop(&mut self) {
            let _ = execute!(stdout(), cursor::Show);
        }
    }

    let model_names: Vec<String> = all_models.iter().map(|m| m.name.clone()).collect();

    let mut search_query = String::new();
    let mut filtered_models = model_names.clone();
    let mut current_selection = 0;
    let window_size = 5;

    enable_raw_mode().context("failed to enable raw mode")?;
    execute!(stdout(), cursor::Hide)?;

    let _cursor_guard = CursorGuard;

    let result = loop {
        let terminal_width = match size() {
            Ok((width, _)) => (width as usize).min(100),
            Err(_) => 80,
        };

        let max_name_width = terminal_width.saturating_sub(10);

        execute!(
            stdout(),
            terminal::Clear(terminal::ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        let mut lines = Vec::new();
        lines.push(format!(
            "{} {}",
            style("🔍").dim(),
            style("intelligent model search").cyan().bold()
        ));
        lines.push(format!(
            "{} {} {}",
            style("📊").dim(),
            style(all_models.len()).yellow().bold(),
            style("total models available").dim()
        ));

        if !search_query.is_empty() {
            lines.push(format!(
                "{} {} {}",
                style("🎯").dim(),
                style("filtering by:").dim(),
                style(format!("'{search_query}'")).green().bold()
            ));
        }

        lines.push(format!(
            "{} {} {}",
            style("✨").dim(),
            style(filtered_models.len()).yellow().bold(),
            style("matches found").dim()
        ));
        lines.push("".to_string());

        if filtered_models.is_empty() {
            lines.push(style("no models match your search").red().dim().to_string());
            lines.push(style("type to search, esc to go back").dim().to_string());
        } else {
            current_selection = current_selection.min(filtered_models.len().saturating_sub(1));
            let (window_start, window_end) =
                calculate_sliding_window(current_selection, filtered_models.len(), window_size);
            let visible_models = &filtered_models[window_start..window_end];

            lines.push(format!(
                "{} {}-{} {} {}",
                style("showing").dim(),
                style(window_start + 1).cyan(),
                style(window_end).cyan(),
                style("of").dim(),
                style(filtered_models.len()).cyan()
            ));
            lines.push("".to_string());

            for (i, model_name) in visible_models.iter().enumerate() {
                let absolute_index = window_start + i;
                let is_current = absolute_index == current_selection;

                let display_name = if model_name.len() > max_name_width {
                    format!("{}...", &model_name[..max_name_width.saturating_sub(3)])
                } else {
                    model_name.clone()
                };

                let formatted_name = if let Some(slash_pos) = display_name.find('/') {
                    let (provider, model) = display_name.split_at(slash_pos);
                    let model = &model[1..];

                    if is_current {
                        format!(
                            "{}{}{}",
                            style(provider).cyan().bold(),
                            style("/").dim(),
                            style(model).white().bold()
                        )
                    } else {
                        format!(
                            "{}{}{}",
                            style(provider).blue().dim(),
                            style("/").dim(),
                            style(model).white().dim()
                        )
                    }
                } else if is_current {
                    style(display_name).white().bold().to_string()
                } else {
                    style(display_name).white().dim().to_string()
                };

                if is_current {
                    lines.push(format!("{} {}", style("►").green().bold(), formatted_name));
                } else {
                    lines.push(format!("  {formatted_name}"));
                }
            }
        }

        lines.push("".to_string());

        let search_display = if search_query.len() > max_name_width.saturating_sub(10) {
            format!("{}...", &search_query[..max_name_width.saturating_sub(13)])
        } else {
            search_query.clone()
        };

        let cursor = if !search_query.is_empty() || filtered_models.is_empty() {
            style("█").green().to_string()
        } else {
            "".to_string()
        };

        let search_line = format!(
            "{} {}{}",
            style("search:").cyan().bold(),
            style(search_display).white(),
            cursor
        );
        lines.push(search_line);
        lines.push("".to_string());

        if !filtered_models.is_empty() {
            lines.push(format!(
                "{} {} {} {} {} {} {} {}",
                style("↑↓").green().bold(),
                style("navigate").dim(),
                style("•").dim(),
                style("enter").green().bold(),
                style("select").dim(),
                style("•").dim(),
                style("esc").red().bold(),
                style("back").dim()
            ));
        } else {
            lines.push(format!(
                "{} {} {} {} {}",
                style("type").green().bold(),
                style("to search").dim(),
                style("•").dim(),
                style("esc").red().bold(),
                style("back").dim()
            ));
        }

        for line in lines {
            println!("\r{line}");
        }

        stdout().flush()?;

        if let Event::Key(KeyEvent { code, .. }) = event::read()? {
            match code {
                KeyCode::Up => {
                    if !filtered_models.is_empty() && current_selection > 0 {
                        current_selection -= 1;
                    }
                }
                KeyCode::Down => {
                    if !filtered_models.is_empty() && current_selection < filtered_models.len() - 1
                    {
                        current_selection += 1;
                    }
                }
                KeyCode::Enter => {
                    if !filtered_models.is_empty() {
                        let selected_model = filtered_models[current_selection].clone();
                        break Ok(selected_model);
                    } else if !search_query.is_empty() {
                        search_query.clear();
                        filtered_models = model_names.clone();
                        current_selection = 0;
                    }
                }
                KeyCode::Esc => break Err(anyhow::anyhow!("cancelled")),
                KeyCode::Backspace => {
                    if !search_query.is_empty() {
                        search_query.pop();
                        filtered_models = intelligent_filter(&model_names, &search_query);
                        current_selection = 0;
                    }
                }
                KeyCode::Char(c) => {
                    search_query.push(c);
                    filtered_models = intelligent_filter(&model_names, &search_query);
                    current_selection = 0;
                }
                _ => {}
            }
        }
    };

    disable_raw_mode().context("failed to disable raw mode")?;

    execute!(
        stdout(),
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    match &result {
        Ok(selected) => {
            let formatted_selected = if let Some(slash_pos) = selected.find('/') {
                let (provider, model) = selected.split_at(slash_pos);
                let model = &model[1..];
                format!(
                    "{}{}{}",
                    style(provider).cyan(),
                    style("/").dim(),
                    style(model).white().bold()
                )
            } else {
                style(selected).white().bold().to_string()
            };

            println!("{} {}", style("✅ selected:").green().bold(), formatted_selected);
        }
        Err(_) => {
            println!("{} {}", style("❌").red(), style("cancelled").dim());
        }
    }

    result
}

fn calculate_sliding_window(
    current_selection: usize,
    total_items: usize,
    window_size: usize,
) -> (usize, usize) {
    if total_items <= window_size {
        return (0, total_items);
    }

    let half_window = window_size / 2;

    let start = if current_selection < half_window {
        0
    } else if current_selection + half_window >= total_items {
        total_items - window_size
    } else {
        current_selection - half_window
    };

    let end = std::cmp::min(start + window_size, total_items);

    (start, end)
}

fn intelligent_filter(models: &[String], query: &str) -> Vec<String> {
    if query.trim().is_empty() {
        return models.to_vec();
    }

    let query_lower = query.to_lowercase();
    let query_parts: Vec<&str> = query_lower.split_whitespace().collect();

    let mut scored_models: Vec<(String, i32)> = models
        .iter()
        .filter_map(|model| {
            let model_lower = model.to_lowercase();
            let score = calculate_match_score(&model_lower, &query_lower, &query_parts);
            if score > 0 {
                Some((model.clone(), score))
            } else {
                None
            }
        })
        .collect();

    scored_models.sort_by(|a, b| b.1.cmp(&a.1));

    scored_models.into_iter().map(|(model, _)| model).collect()
}

fn calculate_match_score(model: &str, query: &str, query_parts: &[&str]) -> i32 {
    let mut score = 0;

    if model == query {
        return 1000;
    }

    if model.starts_with(query) {
        score += 500;
    }

    if model.contains(query) {
        score += 200;
    }

    for part in query_parts {
        if model.contains(part) {
            score += 100;
        }
    }

    score
}
