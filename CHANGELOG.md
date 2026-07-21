## [unreleased]

### Miscellaneous Tasks

- Add GitHub Actions workflow with caching
- Apply cargo fmt code formatting
- Remove clippy and tests from release workflow
- Upgrade actions/checkout to v7
- Migrate to reusable Rust workflow via commit SHA
- Fix release workflow
## [0.6.0] - 2026-06-01

### Bug Fixes

- *(cmd)* Handle non-TTY password prompt gracefully
- *(cmd)* Handle serialization errors properly
- *(ui)* Handle terminal read_key error gracefully
- *(ui)* Prevent infinite loop when input channel disconnects
- *(ui)* Replace f64::INFINITY cast with usize::MAX

### Miscellaneous Tasks

- Format source code with cargo fmt
- Apply clippy suggestions for paths, closures, and casts
- Upgrade project to Rust 2024 edition

### Other

- Version to 0.6.0

### Refactor

- *(cmd)* Use idiomatic combinators for password resolution
- *(ui)* Simplify thread signaling with mpsc channel
- Add clippy check to release workflow

### Testing

- Remove needless array borrows in CLI integration tests
## [0.5.0] - 2026-05-20

### Miscellaneous Tasks

- Update dependencies
- Bump version to 0.5.0
- Synchronize Cargo.lock with Cargo.toml
## [0.4.1] - 2026-02-09

### Bug Fixes

- Update app version by refreshing Cargo.lock

### Miscellaneous Tasks

- *(release)* Bump version to v0.4.0
- *(release)* Bump version to v0.4.1
## [0.4.0] - 2026-02-05

### Bug Fixes

- *(migrate)* Add legacy bypass for migration command
- *(list)* Fixed single-value print for pipelining
- *(tests)* Added -q flag for rename test
- *(tests)* Added -q flag for migration test
- *(tests)* Removed -q flag from migration test

### Documentation

- *(readme)* Update used crates

### Features

- [**breaking**] Deprecate legacy format and require migration
- *(ls)* Add exact match filtering with --exact flag

### Miscellaneous Tasks

- Remove unused code

### Other

- Remove unused rand crate

### Refactor

- *(io)* Use stream deserialization to fix missing newline bug #53 #54
- Update alias_exists to use robust codex reader
- Improve file write safety and consistency

### Testing

- Remove deprecated code, update tests to use alias as positional arg
- Add migration and legacy format tests
- Verify robust parsing of mashed JSON records
## [0.3.1] - 2026-01-15

### Features

- Initial implementation of live OTP list
- "Press any key to exit" prompt for live OTP list
- Safe exits for live OTP list
- Make alias a positional argument for the ls command
- Make alias a positional argument for all commands

### Miscellaneous Tasks

- *(release)* Bump version to v0.3.1

### Other

- Removed trimming of data before being appended to codex, since it resulted in the data to be all written into the first line making ls not work
- Fix first write to codex
## [0.3.0] - 2026-01-11

### Documentation

- Add  flag description

### Miscellaneous Tasks

- Update actions

### Other

- Update app version to 0.3.0, update crates
## [ci/update-actions] - 2026-01-11

### Bug Fixes

- *(cmd)* Implement lazy password prompting in 'ls' command
- *(add)* Use sanitized secret for final OTP generation
- *(ls)* Exit with error code when alias is not found

### Documentation

- Add warning message for password arg
- Update README

### Features

- Add serde dependencies and Record data struct #17
- Add constructor logic and timestamp generation for Record #17
- Migrate storage to JSON with legacy backward compatibility
- *(cmd)* Add migrate command to convert legacy data to JSON #17
- Implement automatic routine backups for write operations #19
- *(ls)* Add `--format json` flag for machine-readable output #21
- *(ls)* Add time-remaining to table and JSON outputs #28
- *(cmd)* Add rename subcommand to update record aliases #22
- *(cli)* Add global `--path` flag for custom codex locations #30
- *(ls)* Add system time diagnostic to help debug clock drift #33
- *(ls)* Upgrade exact match to case-insensitive partial search #34
- *(ui)* Add minimalist progress bar for OTP codes #26

### Miscellaneous Tasks

- Format code with cargo fmt
- *(test)* Rename integration_tests to cli.rs #24

### Refactor

- Create a struct `EncryptArgs` to avoid code duplication #9
- *(config)* Simplify path display logic #12
- *(cmd)* Make `update_code` safer and more efficient
- Move Record struct to dedicated models module #17
- Move command matching into a dedicated `run()` fn
- Restructure CLI args
- Refactor `add` subcommand to return Result and better Err handling
- Introduce OtpError and centralize timestamp logic
- Update generate_otp to use Result and OtpError
- Split crypt into separate encrypt and decrypt functions
- Extract the path logic to a `resolve_codex_path` helper fn
- Clean up main and command dispatch logic #40
- *(file)* Generalize path handling and clean up I/O logic #42
- *(cmd)* Modernize command logic and presentation #44
- *(cmd)* Extract OTP calculation logic from print functions
- *(main)* Use guard clause for config command

### Testing

- *(cli)* Fix usage string match and add hermes helper
- *(cli)* Add `add_remove_isolated_flow` fn
- *(cli)* Refactor update flow
- *(cli)* Add `ls_partial_search_isolated` test to validate search
- *(cli)* Add integration test for JSON output format
## [0.1.0] - 2023-10-13
