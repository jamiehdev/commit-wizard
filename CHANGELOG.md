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
