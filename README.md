<div align="center">

# Hermes-OTP

**A TOTP manager for your terminal and your pipelines.**

[![CI](https://github.com/riccione/hermes-otp/actions/workflows/ci.yml/badge.svg)](https://github.com/riccione/hermes-otp/actions)
[![Release](https://img.shields.io/github/v/release/riccione/hermes-otp)](https://github.com/riccione/hermes-otp/releases)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>

Most TOTP managers are either graphical vaults or throwaway shell scripts.
**Hermes-OTP** is a single, lightweight binary that is comfortable as a daily
driver and well behaved in your CI/CD pipelines.

No runtime, no background daemon, no fragile wrapper.

## Demo

```console
$ hermes-otp add github -c JBSWY3DPEHPK3PXP
Record saved.
621555

$ hermes-otp ls
Alias           | OTP        | Rem
----------------|------------|-----
github          | 621555     | 22s  [##############......]
aws             | 448915     | 22s  [##############......]
Press any key to exit

$ hermes-otp ls -q github
621555
```

The live view refreshes once a second and exits on any keypress. `-q` skips the
countdown bar and the prompt, so it is safe to pipe.

## Why Hermes-OTP?

- **Script-first:** native JSON output, clean stdout/stderr separation, and
  standard exit codes.
- **Full RFC 6238 support:** SHA-1, SHA-256 and SHA-512 records with custom
  digits (6–8) and periods (1–300 s), added directly or imported.
- **Encrypted at rest:** each record is encrypted independently with AES-256-CBC;
  the codex is `0600` and its directory is `0700`.
- **Single ~1 MB binary:** no runtime, no interpreter, no GPG dependency.
- **Live when you want it, quiet when you don't:** an auto-refreshing view for
  humans and a strict non-interactive mode for machines.

### How it compares

| Feature | hermes-otp | oathtool | totp-cli | dotp | pass-otp |
|---|---|---|---|---|---|
| Encrypted storage | Yes (AES-256-CBC) | No | Yes (age) | Yes (Fernet) | Yes (GPG) |
| **JSON output** | **Yes** | No | No | No | No |
| **`otpauth://` import** | **Yes** | No | No (YAML only) | Yes | Yes |
| **Custom algorithm/digits/period** | **Yes** | Yes (per call) | digits only | — | — |
| Single binary | Yes (~1 MB) | Yes | Yes (Go) | No (Python) | No (Shell) |
| Fuzzy search | Yes | No | No | Yes | No |

*Based on each project's published documentation, October 2026. `oathtool` is
stateless and does not manage a vault of secrets.*

## Install

**Linux (x86_64):**

```sh
curl -LO https://github.com/riccione/hermes-otp/releases/latest/download/hermes-otp_linux-x86_64
chmod +x hermes-otp_linux-x86_64
sudo mv hermes-otp_linux-x86_64 /usr/local/bin/hermes-otp
```

**macOS:**

```sh
# Apple Silicon
curl -LO https://github.com/riccione/hermes-otp/releases/latest/download/hermes-otp_macos-arm64
chmod +x hermes-otp_macos-arm64
sudo mv hermes-otp_macos-arm64 /usr/local/bin/hermes-otp

# Intel
curl -LO https://github.com/riccione/hermes-otp/releases/latest/download/hermes-otp_macos-x86_64
chmod +x hermes-otp_macos-x86_64
sudo mv hermes-otp_macos-x86_64 /usr/local/bin/hermes-otp
```

**Windows:** download `hermes-otp_win-x86_64.exe` from the
[releases page](https://github.com/riccione/hermes-otp/releases) and put it on
your `PATH`.

Every release also publishes `SHA256SUMS`.

**With Cargo:**

```sh
cargo install --git https://github.com/riccione/hermes-otp
```

**From source** (Rust 1.97.1+):

```sh
git clone https://github.com/riccione/hermes-otp
cd hermes-otp
cargo build --release
```

## Quick Start

```sh
# Standard record: SHA-1, 6 digits, 30 second step (the defaults)
hermes-otp add github -c JBSWY3DPEHPK3PXP

# Custom record: SHA-256, 8 digits, 60 second step
hermes-otp add enterprise -c JBSWY3DPEHPK3PXP --algorithm sha256 --digits 8 --period 60

# Import an otpauth:// URI (the URI carries its own parameters)
hermes-otp add -c "otpauth://totp/ACME:alice?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256&digits=8"
```

## Scripting & Automation

Hermes-OTP is designed not to break your scripts.

**A bare code:**

```sh
OTP=$(hermes-otp ls -q github)
curl -H "X-OTP: $OTP" https://api.example.com
```

**Structured data:**

```sh
# A single alias returns one JSON object
hermes-otp ls -q -f json github | jq -r .otp

# Listing everything returns an array
hermes-otp ls -q -f json | jq -r '.[].otp'
```

**Passwords in CI** — never hardcode them; use the environment variable:

```yaml
- run: echo "OTP=$(hermes-otp ls -q github)" >> "$GITHUB_OUTPUT"
  env:
    HERMES_OTP_PASSWORD: ${{ secrets.VAULT_PASSWORD }}
```

### Exit codes

| Code | Meaning |
|---|---|
| `0` | Success — every requested record produced a code. |
| `1` | Total failure — wrong password, missing record, IO error. |
| `2` | Usage error — a bad flag or argument. |
| `3` | Partial success — some records produced a code, some did not. |

Exit code `3` exists because records can have different passwords; the process
tells you that the output is incomplete rather than silently giving you a
half-empty result.

## Security

We believe in being honest about cryptography.

- **Per-record encryption:** every record is encrypted on its own, so one
  compromised password does not unlock the whole file.
- **File permissions:** the codex is `0600`; the app's config directory is
  `0700`.
- **Update protection:** changing a record verifies its password *before*
  anything is written, so a typo cannot re-key or corrupt the file.
- **Passwords are never stored:** resolution order is `--password` →
  `HERMES_OTP_PASSWORD` → interactive prompt.

### Limitations to know

- **Deterministic IV:** a fixed zero IV means identical secrets encrypt to
  identical ciphertexts. Fine for local use, not for hostile multi-tenant
  environments.
- **No KDF:** the key is a direct SHA-256 hash of the password rather than a
  memory-hard KDF such as Argon2. Use a strong, unique password.

<details>
<summary>Command reference</summary>

### Commands

| Command | Description |
|---|---|
| `add <ALIAS> -c <CODE>` | Add a record (Base32 secret or `otpauth://` URI). |
| `add --otpauth <URI>` | Add from an `otpauth://` URI. |
| `add --import <FILE>` | Batch import URIs, one per line. |
| `update <ALIAS> -c <CODE>` | Replace a record's secret after verifying its password. |
| `remove <ALIAS>` | Remove a record. |
| `rename <OLD> <NEW>` | Rename an alias. |
| `ls [ALIAS]` | List codes; a partial match filters, `--exact` is case-sensitive. |
| `config` | Show the codex path. |
| `export -o <FILE>` | Export records to a JSON file. |
| `import <FILE>` | Import records from a JSON file. |

### Flags

| Flag | Description |
|---|---|
| `-c, --code` | Base32 secret, or an `otpauth://` URI. |
| `-a, --algorithm` | `sha1` (default), `sha256`, `sha512`. |
| `-d, --digits` | 6–8 digits (default `6`). |
| `-t, --period` | 1–300 seconds (default `30`). |
| `-p, --path` | Custom path to the codex file. |
| `-u, --unencrypt` | Store the secret in plain text (debugging only). |
| `--password` | Password on the command line (leaks into shell history). |
| `-q, --quiet` | Plain output for scripts: no countdown bar or prompt. |
| `-e, --exact` | Exact, case-sensitive alias match for `ls`. |
| `-z, --fuzzy` | Fuzzy search across aliases. |
| `-f, --format` | `table` (default) or `json`. |

### Environment variables

| Variable | Purpose |
|---|---|
| `HERMES_OTP_PASSWORD` | Password used to encrypt and decrypt secrets. |
| `HERMES_OTP_PATH` | Path to the codex file. |

`--path` overrides `HERMES_OTP_PATH`; both override the default
`~/.config/hermes-otp/codex`.

### Copy a code to the clipboard

```sh
# Wayland
hermes-otp ls -q github | wl-copy

# X11
hermes-otp ls -q github | xclip -selection clipboard

# macOS
hermes-otp ls -q github | pbcopy
```

### Import and export

```sh
# Single URI
hermes-otp add --otpauth "otpauth://totp/Example:alice?secret=JBSWY3DPEHPK3PXP&issuer=Example"

# Batch: one URI per line, blank lines ignored, all-or-nothing
hermes-otp add --import uris.txt

# JSON backup and restore
hermes-otp export -o backup.json
hermes-otp import backup.json
```

The JSON format is an array of records:

```json
[
  {
    "alias": "GitHub:alice",
    "secret": "JBSWY3DPEHPK3PXP",
    "is_unencrypted": true,
    "algorithm": "sha1",
    "digits": 6,
    "period": 30,
    "created_at": 1724110000
  }
]
```

`export` writes the secret exactly as stored, so ciphertext stays ciphertext: a
backup is only restorable with the same password.

</details>

## Development

```sh
cargo build --release
cargo test
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Contributions are welcome — open an issue or a pull request.

## License

MIT — see [LICENSE](LICENSE).
