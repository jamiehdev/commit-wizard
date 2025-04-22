# commit wizard

an ai-powered conventional commit message generator for git repositories.

## overview

commit wizard analyses your git diff and uses ai to generate a well-formatted [conventional commit](https://www.conventionalcommits.org/) message based on your changes. it intelligently ignores large files, minified files, and binary files to focus on meaningful code changes.

## features

- 🧠 ai-powered commit message generation
- 📏 follows the conventional commits specification
- 🔍 analyses git diff to understand your changes
- 🥷 ignores large files, minified files, and binary files
- 🔄 works with any git repository
- 💻 simple cli interface

## installation

### prerequisites

- rust and cargo
- git
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
```

##### windows

1. install git:
   - download and install from [git-scm.com](https://git-scm.com/download/win)

2. install rust and cargo:
   - download and run rustup-init.exe from [rustup.rs](https://rustup.rs/)
   - follow the onscreen instructions

##### getting an openrouter api key

1. visit [openrouter.ai](https://openrouter.ai/) and create an account
2. generate a new api key from your account dashboard
3. save this key as you'll need it to use commit-wizard

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
export OPENROUTER_MODEL=microsoft/mai-ds-r1:free  # optional
```

or create a `.env` file in the project directory:
```
OPENROUTER_API_KEY=your-api-key
OPENROUTER_MODEL=microsoft/mai-ds-r1:free
```

3. build the project:
```
cargo build --release
```

4. install the binary (optional):
```
cargo install --path .
```

## usage

basic usage (in a git repository with staged changes):

```
commit-wizard
```

### options

```
options:
  -p, --path <PATH>            path to git repository (defaults to current directory)
  -m, --max-size <MAX_SIZE>    maximum file size in KB to analyse [default: 100]
  -f, --max-files <MAX_FILES>  maximum number of files to analyse [default: 10]
  -v, --verbose                show detailed diff information
  -h, --help                   print help
  -V, --version                print version
```

### examples

```
# generate commit message for current directory
commit-wizard

# generate commit message for specific repository
commit-wizard --path /path/to/repo

# generate commit message with detailed output
commit-wizard --verbose

# analyse larger files (up to 500KB)
commit-wizard --max-size 500

# analyse more files (up to 20)
commit-wizard --max-files 20
```

## how it works

1. commit wizard analyses your git diff to understand the changes
2. it filters out binary files, minified files, and large files
3. it sends the relevant changes to the ai model
4. the ai generates a conventional commit message based on your changes
5. the tool presents the commit message ready to use

## license

mit
