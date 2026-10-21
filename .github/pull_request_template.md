## Why

<!-- The purpose of this PR and the problem it solves. Link to the issue if applicable. -->
Closes #

## What changed

<!-- A short summary of the changes. -->

## Technical details

<!-- Brief notes on the approach, key files, and trade-offs. -->

## Type of change

- [ ] Bug fix (non-breaking change which fixes an issue)
- [ ] New feature (non-breaking change which adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Refactor / CI / Dependency update

## Checklist

- [ ] `cargo fmt`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test`
- [ ] `cargo check`

### Project-Specific Checks
- [ ] **Security:** If this PR touches file I/O, did you ensure files are still created with `0600` and directories with `0700` permissions?
- [ ] **Scripting:** If this PR touches the `ls` command or CLI flags, does it preserve the strict exit codes (`0` success, `1` failure, `3` partial) and valid JSON output?
- [ ] **Documentation:** If this PR adds or changes a CLI flag/behavior, has the `README.md` been updated?
