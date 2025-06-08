<h1 align="center">commit wizard 🧙</h1>
<p align="center">ai-powered conventional commit message generator</p>

<p align="center"><code>npm i -g @jamiehdev/commit-wizard</code></p>

<div align="center">
  <img src="./.github/demo.gif" alt="Commit Wizard demo GIF showing: commit-wizard" />
</div>

---

<details>
<summary><strong>Table of Contents</strong></summary>

<!-- Begin ToC -->

- [quickstart](#quickstart)
- [features](#features)
- [system requirements](#system-requirements)
- [cli reference](#cli-reference)
- [configuration guide](#configuration-guide)
  - [environment variables setup](#environment-variables-setup)
  - [model configuration](#model-configuration)
  - [example configurations](#example-configurations)
- [usage examples](#usage-examples)
- [conventional commits compliance](#conventional-commits-compliance)
- [supported ai providers](#supported-ai-providers)
- [faq](#faq)
- [contributing](#contributing)
  - [development workflow](#development-workflow)
  - [building from source](#building-from-source)
- [security & responsible ai](#security--responsible-ai)
- [license](#license)

<!-- End ToC -->

</details>

---

## quickstart

install via npm:

```shell
npm i -g @jamiehdev/commit-wizard
```

set your OpenRouter API key:

```shell
export OPENROUTER_API_KEY="your-api-key-here"
```

> **note:** you can also place your API key into a `.env` file at the root of your project:
>
> ```env
> OPENROUTER_API_KEY=your-api-key-here
> OPENROUTER_MODEL=deepseek/deepseek-r1-0528:free
> ```

make some changes in your git repository, stage them, and run:

```shell
commit-wizard
```

that's it! commit wizard will:
- analyse your staged changes (or unstaged if nothing is staged)
- detect patterns across 15+ change types (new features, refactoring, api changes, etc.)
- automatically select the optimal ai model based on complexity
- send meaningful code changes to ai (filtering out noise)
- generate conventional commit messages with proper multi-line bodies for complex changes
- let you review, edit, or regenerate the message
- commit your changes with the perfect message

---

## features

commit wizard is built for developers who want **consistent, meaningful commit messages** without the mental overhead. It understands your code changes and generates **conventional commits** that make your git history readable and tooling-friendly.

| feature | description |
|---------|-------------|
| **intelligent pattern detection** | analyses 15+ different change patterns including new features, refactoring, api changes, and cross-layer modifications. |
| **smart model selection** | automatically chooses the optimal ai model based on commit complexity - fast models for simple changes, advanced models for complex ones. |
| **context-aware diff analysis** | sends meaningful code changes to ai (up to 2000 lines) whilst filtering out auto-generated files, lock files, and binary content. |
| **conventional commits** | generates perfectly formatted conventional commit messages following the 1.0 specification. |
| **ai-generated scopes** | creates contextual scopes based on what code sections actually changed - no predefined lists. |
| **multi-line commit bodies** | generates detailed bullet-point explanations for complex changes with proper capitalisation and uk spelling. |
| **interactive workflow** | review, edit, regenerate, or commit with confidence. |
| **performance optimised** | cached regex patterns, efficient string matching, and smart analysis prioritisation. |
| **debug mode** | see the full ai analysis, pattern detection, and model reasoning with `--debug`. |
| **multiple providers** | supports openrouter, openai, deepseek, and other providers. |
| **format validation** | ensures all messages follow conventional commits specification with breaking change support (`!` syntax). |

---

## system requirements

| requirement | details |
|-------------|---------|
| operating systems | macos, linux, windows |
| node.js | 16+ (for npm installation) |
| git | any recent version |
| ai api key | openrouter (recommended) or openai |

---

## cli reference

| command | purpose |
|---------|---------|
| `commit-wizard` | interactive commit message generation |
| `commit-wizard --debug` | show detailed ai analysis and reasoning |
| `commit-wizard --yes` | auto-commit without confirmation |
| `commit-wizard --verbose` | show detailed file change information |
| `commit-wizard --help` | show all available options |

### key flags

| flag | short | description |
|------|-------|-------------|
| `--path <PATH>` | `-p` | specify git repository path (defaults to current directory) |
| `--max-size <KB>` | | maximum file size to analyse in kb (default: 100) |
| `--max-files <NUM>` | `-f` | maximum number of files to analyse (default: 10) |
| `--verbose` | `-v` | show detailed diff information |
| `--yes` | `-y` | automatically commit when confirmed |
| `--debug` | | show debug information including raw ai responses and model selection reasoning |
| `--smart-model` | | enable intelligent model selection based on commit complexity |

### model settings

commit wizard includes an interactive model settings menu accessible during the commit process:

- **change model** - browse and select from 20+ pre-configured models
- **auto-complexity** - let commit wizard choose the optimal model automatically
- **search models** - find specific models from the full openrouter catalogue
- **save preferences** - persist your model choices to `~/.config/commit-wizard/config.toml`

**access model settings:** during the commit workflow, select "change model settings" from the main menu.

### example output:

**simple changes** get concise single-line messages:
```bash
$ commit-wizard

🧙 analysing commit changes...
🧙 generating commit message with deepseek/deepseek-r1-0528:free...

✅ generated commit message:

fix(parser): handle edge case in regex pattern matching

? what would you like to do? ›
❯ yes, commit this message
  edit this message  
  no, regenerate message
```

**complex changes** get detailed multi-line explanations:
```bash
$ commit-wizard

🧙 analysing commit changes...
🧙 generating commit message with anthropic/claude-3.5-sonnet...

✅ generated commit message:

feat(ai): implement intelligent commit analysis with pattern detection

- Add CommitIntelligence struct with 15 distinct change pattern types
- Implement smart model selection based on complexity scoring
- Add context-aware diff filtering excluding auto-generated files
- Integrate performance optimisations with cached regex patterns
- Support multi-line commit bodies with proper uk spelling

? what would you like to do? ›
❯ yes, commit this message
  edit this message  
  no, regenerate message
```

---

## configuration guide

### environment variables setup

commit wizard uses environment variables for configuration. you can set these in your shell or in a `.env` file in your project root.

| variable | required | description | example |
|----------|----------|-------------|---------|
| `OPENROUTER_API_KEY` | yes | your openrouter api key | `sk-or-v1-...` |
| `OPENROUTER_MODEL` | no | ai model to use | `deepseek/deepseek-r1-0528:free` |

### model configuration

commit wizard uses `deepseek/deepseek-r1-0528:free` as the default model, providing excellent code analysis at no cost. you can customise model selection through:

1. **environment variable:** `OPENROUTER_MODEL=your-preferred-model`
2. **interactive menu:** accessible during the commit workflow
3. **config file:** automatically saved to `~/.config/commit-wizard/config.toml`
4. **smart selection:** use `--smart-model` for automatic complexity-based choice

#### pre-configured models:

| tier | model | description | use case |
|------|-------|-------------|----------|
| **free** | `deepseek/deepseek-r1-0528:free` | thinking model with excellent code understanding | complex commits, default choice |
| **free** | `deepseek/deepseek-chat-v3-0324:free` | fast model for quick analysis | simple commits, speed priority |
| **free** | `meta-llama/llama-3.1-8b-instruct:free` | solid general performance | balanced free option |
| **premium** | `anthropic/claude-3.5-sonnet` | superior reasoning and context | complex codebases, best quality |
| **premium** | `openai/gpt-4o` | balanced performance and speed | general use, reliable choice |
| **premium** | `openai/gpt-4o-mini` | cost-effective openai option | budget-conscious, frequent use |

#### smart model selection

when enabled with `--smart-model`, commit wizard automatically chooses:
- **fast models** for simple changes (single files, small modifications)
- **thinking models** for complex changes (multiple files, architectural changes, new features)

### example configurations

**.env file (recommended):**
```env
OPENROUTER_API_KEY=sk-or-v1-your-key-here
OPENROUTER_MODEL=deepseek/deepseek-r1-0528:free
```

**shell configuration:**
```bash
# Add to your ~/.bashrc, ~/.zshrc, etc.
export OPENROUTER_API_KEY="sk-or-v1-your-key-here"
export OPENROUTER_MODEL="deepseek/deepseek-r1-0528:free"
```

---

## conventional commits compliance

commit wizard follows the [conventional commits](https://www.conventionalcommits.org/) specification strictly:

### format
```
<type>[optional scope]: <description>

[optional body]

[optional footer(s)]
```

### supported types
- `feat`: new feature
- `fix`: bug fix
- `docs`: documentation changes
- `style`: formatting changes (no code change)
- `refactor`: code change that neither fixes bug nor adds feature
- `perf`: performance improvements
- `test`: adding/correcting tests
- `build`: build system or dependency changes
- `ci`: ci configuration changes
- `chore`: other changes

### smart scope generation
unlike tools with predefined scopes, commit wizard generates **contextual scopes** based on your actual changes:
- `auth` for authentication-related changes
- `parser` for parsing logic updates
- `api` for api endpoint modifications
- `cli` for command-line interface changes
- and many more based on your codebase!

---

## supported ai providers

commit wizard works with any openai-compatible api. popular choices:

| provider | api base | models | notes |
|----------|----------|--------|-------|
| **openrouter** | `https://openrouter.ai/api/v1` | 100+ models | recommended - access to many models |
| **openai** | `https://api.openai.com/v1` | gpt models | direct openai access |
| **deepseek** | `https://api.deepseek.com` | deepseek models | excellent for code analysis |
| **anthropic** | via openrouter | claude models | great reasoning capabilities |

---

## faq

<details>
<summary><strong>how accurate are the generated commit messages?</strong></summary>

> commit wizard v1.1.0 brings significant accuracy improvements with intelligent pattern detection across 15+ change types, context-aware diff analysis that sends actual code content to ai (not just summaries), and smart filtering that excludes noise whilst preserving meaningful changes. the ai now sees exactly what you changed, leading to specific, descriptive commit messages.

</details>

<details>
<summary><strong>can i edit the generated messages?</strong></summary>

> absolutely! the interactive workflow lets you:
> - accept the message as-is
> - edit it in your preferred editor
> - regenerate a completely new message
> - cancel and make manual commits

</details>

<details>
<summary><strong>what if i don't have staged changes?</strong></summary>

> commit wizard will automatically analyse your unstaged changes and warn you. you can stage the changes you want and run it again, or let it analyse everything and then stage before committing.

</details>

<details>
<summary><strong>how much does it cost to use?</strong></summary>

> using openrouter with the default free model (`deepseek/deepseek-r1-0528:free`) costs nothing. premium models have small per-request costs (typically $0.001-0.01 per commit).

</details>

<details>
<summary><strong>does it work with large codebases?</strong></summary>

> absolutely! commit wizard v1.1.0 is optimised for large codebases with intelligent diff filtering:
> - automatically excludes auto-generated files (lock files, node_modules, build artifacts)
> - prioritises source code over tests, config, and documentation
> - sends up to 2000 lines of meaningful changes to ai (perfect for modern 64k+ token models)
> - smart file prioritisation ensures important changes get full context
> - performance optimisations with cached patterns and efficient analysis

</details>

<details>
<summary><strong>can i use it in ci/cd pipelines?</strong></summary>

> yes! use the `--yes` flag for automated commits:
>
> ```bash
> commit-wizard --yes
> ```
>
> perfect for automated dependency updates, code generation, etc.

</details>

---

## contributing

we welcome contributions! whether you're fixing bugs, adding features, or improving documentation.

### development workflow

1. **fork and clone** the repository
2. **create a feature branch** from `main`
3. **make your changes** with tests
4. **run the test suite**: `cargo test`
5. **check formatting**: `cargo fmt --check`
6. **run clippy**: `cargo clippy -- -D warnings`
7. **open a pull request**

### building from source

```bash
# clone the repository
git clone https://github.com/jamiehdev/commit-wizard.git
cd commit-wizard/commit-wizard-cli

# build in development mode
cargo build

# run tests
cargo test

# run with debug output
cargo run -- --debug

# build release version
cargo build --release
```

---

## security & responsible ai

- **api key security**: store api keys in environment variables or `.env` files, never in code
- **privacy**: your code changes are sent to the ai provider for analysis
- **no data retention**: most providers offer zero data retention options
- **local processing**: all git operations happen locally on your machine

for security concerns, please email [jamie@prettypragmatic.com](mailto:jamie@prettypragmatic.com).

---

## license

this project is licensed under the [mit license](LICENSE).

---

<p align="center">
  made with ❤️ for developers who care about commit quality
</p>
