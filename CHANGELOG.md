## v2.5.0

- feat(ci): enhance testing with AI smoke tests and improved workflows

## v2.4.1

- fix(ci): remove skip ci from releases and prevent double-versioning
- refactor(core,ci): major improvements to architecture and workflows
- chore(release): v2.4.0

## v2.4.0

- fix(napi): resolve clippy uninlined format args warnings
- chore(release): v2.3.0
- fix(ci): resolve clippy warnings and add release permissions
- chore(release): v2.2.0
- fix(lint): resolve clippy warnings in security detection logic
- chore(release): v2.1.0
- chore(release): v2.0.0
- feat(ai): improve scope detection and add formatting defense
- chore(release): v1.8.2
- chore(release): v1.8.1
- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v2.3.0

- fix(ci): resolve clippy warnings and add release permissions
- chore(release): v2.2.0
- fix(lint): resolve clippy warnings in security detection logic
- chore(release): v2.1.0
- chore(release): v2.0.0
- feat(ai): improve scope detection and add formatting defense
- chore(release): v1.8.2
- chore(release): v1.8.1
- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v2.2.0

- fix(lint): resolve clippy warnings in security detection logic
- chore(release): v2.1.0
- chore(release): v2.0.0
- feat(ai): improve scope detection and add formatting defense
- chore(release): v1.8.2
- chore(release): v1.8.1
- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v2.1.0

- chore(release): v2.0.0
- feat(ai): improve scope detection and add formatting defense
- chore(release): v1.8.2
- chore(release): v1.8.1
- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v2.0.0

- feat(ai): improve scope detection and add formatting defense

## v1.8.2

- chore(release): v1.8.1
- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v1.8.1

- fix(validation): allow dots in commit message scope names
- build(dependencies): update Cargo dependencies
- chore(release): v1.8.0

## v1.8.0

- fix(ci): handle release-tool's auto-commit behavior correctly
- fix(ci): ensure prepare-release commits and tags changes in CI
- chore: update Cargo.lock for v1.7.0 release
- chore(release): v1.7.0
- fix(ci): remove main branch trigger from ci.yml to eliminate redundancy
- fix(ci): streamline workflows and fix NAPI build ordering
- fix: correct cargo audit syntax in CI workflow
- chore(release): v1.6.0
- feat: restructure CI/CD workflows to eliminate redundant runs
- chore: update Cargo.lock for v1.5.0
- chore(release): v1.5.0
- fix: improve CI/CD pipeline reliability and workflow consistency
- feat: implement version sync script for npm/cargo consistency
- chore(release): v1.4.4
- fix: use inline format args to satisfy Clippy
- chore(release): prepare v1.4.3

## v1.7.0

- fix(ci): remove main branch trigger from ci.yml to eliminate redundancy
- fix(ci): streamline workflows and fix NAPI build ordering
- fix: correct cargo audit syntax in CI workflow
- chore(release): v1.6.0
- feat: restructure CI/CD workflows to eliminate redundant runs
- chore: update Cargo.lock for v1.5.0
- chore(release): v1.5.0
- fix: improve CI/CD pipeline reliability and workflow consistency
- feat: implement version sync script for npm/cargo consistency
- chore(release): v1.4.4
- fix: use inline format args to satisfy Clippy
- chore(release): prepare v1.4.3

## v1.6.0

- feat: restructure CI/CD workflows to eliminate redundant runs
- chore: update Cargo.lock for v1.5.0
- chore(release): v1.5.0
- fix: improve CI/CD pipeline reliability and workflow consistency
- feat: implement version sync script for npm/cargo consistency
- chore(release): v1.4.4
- fix: use inline format args to satisfy Clippy
- chore(release): prepare v1.4.3

## v1.5.0

- fix: improve CI/CD pipeline reliability and workflow consistency
- feat: implement version sync script for npm/cargo consistency
- chore(release): v1.4.4
- fix: use inline format args to satisfy Clippy
- chore(release): prepare v1.4.3

## v1.4.4

- fix: use inline format args to satisfy Clippy
- chore(release): prepare v1.4.3

## v1.4.2

- chore(release): v1.4.0
- chore(ci): update rust toolchain to 1.88.0
- fix: security and performance enhancements

## v1.4.0

- chore(ci): update rust toolchain to 1.88.0
- fix: security and performance enhancements

## v1.4.0-beta

- fix: correct workspace version back to 1.3.3
- chore(bin): format code to remove whitespace and make error message compact
- fix: improve version sync script path resolution and workspace detection
- fix: remove unused import in release_tool.rs
- chore: sync versions after testing version sync script
- feat: add global version sync system for workspace consistency
- chore(release): v1.3.3
- fix(ci): configure git user for release tool commits
- fix(ci): remove invalid --workspace flag from cargo run command
- docs: update changelog with JSON corruption fix

## v1.3.2

- fix(release): fix JSON corruption in package.json version updates
- fix(release): support prerelease versions and stable releases from release branches
- chore: update Cargo.lock for v1.3.2-beta
- chore(release): v1.3.2-beta
- chore(commit-wizard-core): update editor configuration and format print statement
- fix(editor): improve editor selection for mac users
- fix: enhance git security and automate releases with new release_tool
