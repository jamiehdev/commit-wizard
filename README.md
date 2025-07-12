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
| node.js | 20+ (for npm installation) |
| git | any recent version |
| ai api key | openrouter (recommended) or openai |

> **note on windows:** the standalone binary for windows is distributed as a `.tar.gz` archive. you may need a tool like 7-zip or winrar to extract it.

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
<p>very accurate! commit wizard's pattern detection and context-aware analysis mean it understands the intent behind your changes. for best results, ensure your code has meaningful variable and function names.</p>
</details>

<details>
<summary><strong>what happens if i run <code>commit-wizard</code> with no staged changes?</strong></summary>
<p><code>commit-wizard</code> is smart about it. if there are no staged changes, it will look for any unstaged changes in your repository and offer to use those instead. if there are no changes at all (staged or unstaged), it will inform you and exit gracefully.</p>
</details>

<details>
<summary><strong>how does <code>commit-wizard</code> handle very large changes or huge diffs?</strong></summary>
<p><code>commit-wizard</code> has safeguards to handle large changes effectively. it analyses file content up to a certain limit (default 2000 lines per diff) to ensure performance and stay within the ai model's context window. it also limits the number of files it analyzes in a single run. for exceptionally large commits, it's often better to break them down into smaller, logical commits, and <code>commit-wizard</code> encourages this by focusing on a manageable set of changes.</p>
</details>

<details>
<summary><strong>what about binary files or lock files?</strong></summary>
<p><code>commit-wizard</code> is configured to automatically ignore binary files (like images or executables) and lock files (like <code>package-lock.json</code> or <code>Cargo.lock</code>). this ensures that only meaningful code changes are sent to the ai, resulting in more accurate and relevant commit messages.</p>
</details>

<details>
<summary><strong>is my code sent to a third-party server?</strong></summary>
<p>yes, but only the parts that changed. commit wizard sends your git diff to the configured ai provider's api (e.g., openrouter). private or proprietary code should be handled with care. always review your organisation's policies on using external ai tools.</p>
</details>

<details>
<summary><strong>can i edit the generated messages?</strong></summary>
<p>absolutely! the interactive workflow lets you review the generated message and choose to accept it, edit it in your default terminal editor, or regenerate a new one if you're not satisfied.</p>
</details>

<details>
<summary><strong>how much does it cost to use?</strong></summary>
<p>using openrouter with the default free model (`deepseek/deepseek-r1-0528:free`) costs nothing. if you choose to use premium models, there are small per-request costs, but they are generally very low.</p>
</details>

<details>
<summary><strong>can i use it in ci/cd pipelines?</strong></summary>
<p>yes! use the <code>--yes</code> flag (<code>-y</code> for short) for non-interactive, automated commits. this is perfect for scripts, hooks, or CI/CD pipelines where you want to automate commit message generation.</p>
</details>

---

## automated quality assurance

this project is committed to high standards of code quality and release reliability, enforced through a comprehensive, automated ci/cd pipeline. here’s what we check on every change:

| check | tool | description |
|---|---|---|
| **unit & integration tests** | `cargo test` & `npm test` | validates that core rust logic and node.js bindings are working correctly across all supported platforms (linux, macos, windows). |
| **code quality linting** | `clippy` | automatically checks for common rust pitfalls and ensures the code is idiomatic and performant. |
| **security vulnerability scanning** | `cargo audit` | scans all rust dependencies for known security vulnerabilities to prevent supply chain attacks. |
| **commit message linting** | `commitlint` | enforces that every commit in a pull request follows the conventional commits specification. |
| **consistent build environment**| node.js & rust versions | ensures that all builds and tests run in a consistent environment to prevent platform-specific bugs. |
| **automated releases** | github actions & `gh` | creates automated, tagged releases on github and publishes to npm, with artifacts for all platforms. |

---

## contributing

we welcome contributions! whether you're fixing bugs, adding features, or improving documentation.

### release process

this project uses a semi-automated release process driven by github actions. here’s how it works:

1.  **preparation**: for a new release, a maintainer will typically bump the version numbers in `cargo.toml` and `package.json` and run a local script to update the `changelog.md`.
2.  **triggering the release**: the release process is triggered when a new tag is pushed to the repository in the format `v*` (e.g., `v1.4.0`).
3.  **automation**: the `build & release` workflow automatically builds the binaries and napi modules for all platforms, runs tests, and publishes the release to both github and npm.

if you are contributing, you do not need to worry about this process. simply create a pull request with your changes, and a maintainer will handle the release.

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
