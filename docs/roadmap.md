# Hermes-OTP Roadmap

This document outlines the planned development and architectural changes for Hermes-OTP.

## v0.9.0 (Next)
*The "Cleanup" release. (Target: 1-2 months after v0.8.0).*
- [ ] **Drop `magic-crypt` entirely:** Remove the insecure dependency and all legacy decryption logic.
- [ ] **Remove `migrate` command:** It is no longer needed once v1 is dropped.
- [ ] **Hard-fail on v1 records:** If a user skips v0.8.0, they will get a clear error instructing them to run v0.8.0 first to rescue their data.

## v1.0.0 (Future Architecture)
*The "Performance & Ecosystem" release.*
- [ ] **Master Vault Architecture (v3 format):** 
  - *Why:* Currently, `ls` runs Argon2 for *every* record, causing a 1-2 second delay on large vaults. 
  - *How:* Switch to a single randomly generated "Vault Key" that encrypts all records (using fast XChaCha20). The Vault Key itself is encrypted by the Master Password (via Argon2). Argon2 only runs once per command, making `ls` instant.
- [ ] **Publish to crates.io:** Allow users to install via `cargo install hermes-otp`.
- [ ] **Shell Completions:** Add `clap_complete` to generate autocompletion scripts for Bash, Zsh, and Fish.

## Backlog / Nice to Have
- [ ] **Clipboard Support:** Add a `-c` or `--copy` flag to `ls` to automatically copy the code to the clipboard (requires `arboard` crate).
- [ ] **`pass` Compatibility:** Add an import format specifically for `pass-otp` (`pass insert`).
- [ ] **Custom TUI Bindings:** Allow users to press `y` in the live UI to immediately copy the currently selected OTP.

---
*Last updated: October 2026*
