# commit wizard

an ai-powered conventional commit message generator for git repositories.

## overview

commit wizard analyses your git diff and uses ai to generate a well-formatted [conventional commit](https://www.conventionalcommits.org/) message based on your changes. it intelligently ignores large files, minified files, and binary files to focus on meaningful code changes.

## features

- 🧠 ai-powered commit message generation
- 📦 available as an easy-to-install npm package
- 📏 follows the conventional commits specification
- 🔍 analyses git diff to understand your changes
- 🥷 ignores large files, minified files, and binary files
- 🔄 works with any git repository
- 💻 simple cli interface (via `commit-wizard` or `cw` commands)
- 📋 detects and displays staged changes
- ✏️ edit generated commit messages before committing
- 🚀 automatic commit execution with --yes flag
- 🧩 modular architecture with core library, cli, and node.js components

## installation via npm/yarn (recommended)

this is the easiest way to get started with commit wizard.

### prerequisites for npm/yarn installation

- **node.js and npm/yarn**: commit wizard is distributed as an npm package which requires node.js.
  - install node.js (which includes npm) from [nodejs.org](https://nodejs.org/).
  - yarn can be installed via npm: `npm install -g yarn`.
- **git**: ensure git is installed and accessible in your path.
- **text editor**: for editing commit messages - commit wizard will use your system's default editor.
  - it looks for editors set in the `EDITOR` or `VISUAL` environment variables.
  - if none is set, it will try to find common editors (VS Code (`code -w`), nvim, vim, nano, etc.) on your system.
- **openrouter api key**: for the ai functionality.
  1. visit [openrouter.ai](https://openrouter.ai/) and create an account.
  2. generate a new api key from your account dashboard.
  3. set this key as an environment variable (see setup below).

### installing the package

```bash
# using npm
npm install -g @jamiehdev/commit-wizard

# or using yarn
yarn global add @jamiehdev/commit-wizard
```

### setup after installation

set the required environment variable for the ai integration:

```bash
export OPENROUTER_API_KEY="your-api-key"
# optional: specify a model (defaults to a capable free model)
# export OPENROUTER_MODEL="meta-llama/llama-3-70b-instruct:free"
```

you can add these lines to your shell configuration file (e.g., `.bashrc`, `.zshrc`, `.profile`) to make them permanent.

alternatively, commit wizard will also load these variables from a `.env` file in the directory where you run the command, or in any parent directory.
create a `.env` file in your project or home directory with:

```env
OPENROUTER_API_KEY=your-api-key
# OPENROUTER_MODEL=meta-llama/llama-3-70b-instruct:free
```

you can also set your preferred text editor for editing commit messages:

```bash
export EDITOR=nano  # or vim, emacs, code, etc.
```

## usage (npm/yarn installed)

once installed globally, you can run `commit-wizard` or its alias `cw` in any git repository with staged changes:

```bash
# navigate to your git repository
cd /path/to/your/project

# make and stage your changes
git add .

# run commit wizard
commit-wizard
# or
cw
```

### options

the same options apply as when building from source:

```
options:
  -p, --path <PATH>            path to git repository (defaults to current directory)
  -m, --max-size <MAX_SIZE>    maximum file size in kb to analyse [default: 100]
  -f, --max-files <MAX_FILES>  maximum number of files to analyse [default: 10]
  -v, --verbose                show detailed diff information
  -y, --yes                    automatically run the commit command when confirmed
  -h, --help                   print help
  -V, --version                print version
```

### examples (npm/yarn installed)

```bash
# generate commit message for current directory
cw

# generate commit message for specific repository
cw --path /path/to/another/repo

# generate commit message with detailed output and auto-commit
cw --verbose --yes
```

## building from source (for development)

if you want to contribute or build from the source code directly:

### prerequisites

- rust and cargo (see [rustup.rs](https://rustup.rs/))
- git
- node.js and npm (for the npm package)
- text editor (for editing commit messages)
- openrouter api key (for ai functionality)

#### installing prerequisites

##### linux

```bash
# install git
sudo apt update
sudo apt install git  # debian/ubuntu
# or
sudo dnf install git  # fedora
# or
sudo pacman -S git    # arch

# install rust and cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# install node.js and npm (if building the npm package)
sudo apt install nodejs npm  # debian/ubuntu
# or
sudo dnf install nodejs npm  # fedora
# or
sudo pacman -S nodejs npm    # arch
```

##### macos

```bash
# install git
brew install git  # using homebrew
# or install xcode command line tools which includes git
xcode-select --install

# install rust and cargo
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# install node.js and npm (if building the npm package)
brew install node
```

##### windows

1. install git:
   - download and install from [git-scm.com](https://git-scm.com/download/win)

2. install rust and cargo:
   - download and run rustup-init.exe from [rustup.rs](https://rustup.rs/)
   - follow the onscreen instructions

3. install node.js and npm (if building the npm package):
   - download and install from [nodejs.org](https://nodejs.org/)

##### getting an openrouter api key

1. visit [openrouter.ai](https://openrouter.ai/) and create an account
2. generate a new api key from your account dashboard
3. save this key as you'll need it to use commit-wizard

### project structure

commit wizard is organised as a rust workspace with three main components:

- **commit-wizard-core**: the core library containing the main functionality
- **commit-wizard-cli**: the command-line interface
- **commit-wizard-napi**: the node.js binding for npm distribution

### setup

1. clone this repository:
```bash
# https
git clone https://github.com/jamiehdev/commit-wizard.git

# or ssh
git clone git@github.com:jamiehdev/commit-wizard.git

cd commit-wizard
```

2. set environment variables for the ai integration:
```
export OPENROUTER_API_KEY=your-api-key
export OPENROUTER_MODEL=nvidia/llama-3.1-nemotron-ultra-253b-v1:free  # optional
export EDITOR=nano  # optional - set your preferred editor
```

or create a `.env` file in the project directory:
```
OPENROUTER_API_KEY=your-api-key
OPENROUTER_MODEL=nvidia/llama-3.1-nemotron-ultra-253b-v1:free
# optional: set your preferred editor for commit messages
# EDITOR=code -w
```

3. build all components:
```
cargo build --release
```

4. build the npm package (optional):
```
cd commit-wizard-napi
npm run build-all
```

5. install the CLI binary (optional):
```
cargo install --path commit-wizard-cli
```

6. link the npm package locally (optional):
```
cd commit-wizard-napi
npm link
```

## how it works

1. commit wizard checks for staged changes and displays them if found
2. it analyses your git diff to understand the changes
3. it filters out binary files, minified files, and large files
4. it sends the relevant changes to the ai model
5. the ai generates a conventional commit message based on your changes
6. you can accept, edit, or regenerate the suggested commit message
7. with the --yes flag, it can automatically execute the git commit command

## license

mit

