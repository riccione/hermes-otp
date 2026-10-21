<a name="readme-top"></a>

<!-- PROJECT SHIELDS -->
<div align="center">

[![CI](https://github.com/riccione/hermes-otp/actions/workflows/ci.yml/badge.svg)](https://github.com/riccione/hermes-otp/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>
<div align="center">
  <h3 align="center">Hermes-OTP: CLI OTP manager</h3>
</div>

<!-- TABLE OF CONTENTS -->
<details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#getting-started">Getting Started</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#installation">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a>
      <ul>
        <li><a href="#updating-a-record">Updating a record</a></li>
        <li><a href="#exit-codes">Exit codes</a></li>
        <li><a href="#pipeline-integration">Pipeline Integration</a></li>
        <li><a href="#automatically-copy-otp-code-to-clipboard">Clipboard</a></li>
      </ul>
    </li>
    <li><a href="#security">Security</a></li>
    <li><a href="#testing">Testing</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details>

<!-- ABOUT THE PROJECT -->
## About The Project

A CLI OTP manager designed for pipelines and automation. Generates TOTP codes programmatically with JSON output, environment variable support, and non-interactive mode.

Built with:
- [Clap](https://crates.io/crates/clap)
- [Magic-crypt](https://crates.io/crates/magic-crypt)
- [Data-encoding](https://crates.io/crates/data-encoding)
- [Totp-lite](https://crates.io/crates/totp-lite)
- [console](https://crates.io/crates/console)
- [dirs](https://crates.io/crates/dirs)
- [rpassword](https://crates.io/crates/rpassword)
- [serde](https://crates.io/crates/serde)
- [serde_json](https://crates.io/crates/serde_json)
- [url](https://crates.io/crates/url)
- [urlencoding](https://crates.io/crates/urlencoding)
- [nucleo-matcher](https://crates.io/crates/nucleo-matcher)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

### Prerequisites

* Rust 1.85+

### Installation

Build from source:
`cargo build --release`

The binary is located in [target/release/hermes-otp](https://github.com/riccione/hermes-otp/releases).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- USAGE EXAMPLES -->
## Usage

Running without arguments shows help.

There are 2 env variables:

* `HERMES_OTP_PASSWORD`: Password for codes.
* `HERMES_OTP_PATH`: Path to codex file.

Commands:

* `add <ALIAS> -c <CODE> [OPTIONS]`: Add new record.
* `add -c <OTPAUTH_URI> [OPTIONS]`: Add from an otpauth:// URI (no alias needed).
* `add --otpauth <OTPAUTH_URI> [OPTIONS]`: Import from otpauth:// URI.
* `add --import <FILE> [OPTIONS]`: Batch import from file (one URI per line).
* `remove <ALIAS> [OPTIONS]`: Remove record.
* `update <ALIAS> -c <CODE> [OPTIONS]`: Replace the OTP secret of a record. Verifies the record's password first; a wrong password changes nothing.
* `rename <OLD ALIAS> <NEW ALIAS> [OPTIONS]`: Rename alias.
* `ls [OPTIONS]`: Get all OTP codes.
* `ls <ALIAS>`: Get OTP code by alias.
* `ls <PARTIAL MATCH>`: Get OTP codes by partial match. 
* `ls --exact <ALIAS>`: Get OTP codes by exact match (case-sensitive). 
* `config`: Show location of the codex file.
* `export -o <FILE>`: Export all records to a JSON file (backup).
* `import <FILE>`: Import records from a JSON file.

Flags:

* `-c`, `--code`: Base32 secret, or an `otpauth://` URI.
* `-a`, `--algorithm`: Hash algorithm for `add`: `sha1` (default), `sha256`, `sha512`.
* `-d`, `--digits`: Number of digits for `add`: 6 to 8 (default `6`).
* `-t`, `--period`: Time step in seconds for `add`: 1 to 300 (default `30`).
* `-p`, `--path`: Custom path to the codex file.
* `-u`, `--unencrypt`: WARNING: Store the secret in plain text. Use for debugging only.
* `--password`: WARNING: Using this flag leaves password in shell history.
* `-q`, `--quiet`: Only for `ls`. Plain output for scripts: no countdown bar or interactive prompt.
* `-e`, `--exact`: Only for `ls <ALIAS>`. Exact match (case-sensitive).
* `-z`, `--fuzzy`: Only for `ls`. Fuzzy search across aliases (sorted by match score).
* `-f [table, json]`, `--format [table, json]`: Only for `ls` command. Format output as table (default) or as JSON.
* `--otpauth <URI>`: Import from otpauth:// URI.
* `--import <FILE>`: Import multiple URIs from a file.

### Adding a record with custom OTP parameters

Most services use SHA-1, 6 digits and a 30 second step, which is the default:

```sh
hermes-otp add github -c JBSWY3DPEHPK3PXP
```

For services that specify different RFC 6238 parameters, pass them directly
instead of building an `otpauth://` URI:

```sh
hermes-otp add enterprise -c JBSWY3DPEHPK3PXP --algorithm sha256 --digits 8 --period 60
```

| Flag | Default | Accepted values |
|------|---------|-----------------|
| `-a`, `--algorithm` | `sha1` | `sha1`, `sha256`, `sha512` |
| `-d`, `--digits` | `6` | `6` to `8` |
| `-t`, `--period` | `30` | `1` to `300` seconds |

Invalid values are rejected before anything is written:

```
$ hermes-otp add enterprise -c JBSWY3DPEHPK3PXP --digits 5
Error: Unsupported OTP parameter (digits=5). Only 6 to 8 digits are supported.
```

You can also pass an `otpauth://` URI to `-c`; in that case the URI's own
parameters apply, and combining the URI with `--algorithm`, `--digits` or
`--period` is an error.

### Updating a record

```sh
hermes-otp update <ALIAS> -c <NEW_CODE>
```

Prompts for that record's current password and verifies it by decrypting the
record before anything is written. If the password is wrong:

```
Error: Incorrect password. Nothing was changed.
```

The replacement is encrypted with the same password. `-u` stores it in plain
text instead, but the record's password is still required to verify access.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Every matched record produced a code. |
| `1` | Runtime error, or no record could produce a code (wrong password, corrupt secret, no match). |
| `2` | Usage error (invalid flags or arguments). |
| `3` | Partial: some records produced a code, some did not. |

Exit code `3` exists because records can have different passwords; see
[Independent record encryption](#independent-record-encryption).

### Pipeline Integration

Hermes-OTP is designed for scripting and automation. Use `--format json` for machine-readable output and `-q` to suppress interactive prompts.

**Generate OTP in a script:**
```sh
OTP=$(hermes-otp ls my_alias -q --format json | jq -r '.otp')
curl -X POST https://api.example.com/auth -d "otp=$OTP"
```

An alias that matches a single record prints one JSON object; listing all
records, or an alias that matches several, prints an array:

```sh
hermes-otp ls -f json my_alias | jq -r '.otp'     # one object
hermes-otp ls -f json          | jq -r '.[].otp'  # array
```

**Use with environment variables:**
```sh
export HERMES_OTP_PASSWORD="mysecret"
export HERMES_OTP_PATH="/secure/codex.json"
hermes-otp ls my_alias -q
```

**Non-interactive mode (no progress bar):**
```sh
hermes-otp ls my_alias -q | grep -oE '[0-9]{6}'
```

### Automatically copy OTP code to clipboard

**Wayland**
```sh
hermes-otp ls my_alias | wl-copy
```

**X11**
```sh
hermes-otp ls my_alias | xclip -selection clipboard
```

**macOS**
```sh
hermes-otp ls my_alias | pbcopy
```

### Import from otpauth:// URI

Single URI:
```sh
hermes-otp add --otpauth "otpauth://totp/Example:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=Example"
```

Batch import from file (one URI per line):
```sh
hermes-otp add --import /path/to/uris.txt
```

Example file format:
```
otpauth://totp/GitHub:alice?secret=JBSWY3DPEHPK3PXP&issuer=GitHub
otpauth://totp/Google:bob?secret=ONSWY3DPEHPK3PXP&issuer=Google
```

The RFC 6238 parameters are honored, not just the secret:

| Parameter | Default | Accepted values |
|-----------|---------|-----------------|
| `secret` | *(required)* | Base32 |
| `issuer` | none | any string |
| `algorithm` | `sha1` | `sha1`, `sha256`, `sha512` (case-insensitive) |
| `digits` | `6` | `6` to `8` |
| `period` | `30` | `1` to `300` seconds |

Invalid values are rejected at import instead of silently producing wrong
codes:

```
$ hermes-otp add --otpauth "otpauth://totp/App:user?secret=JBSWY3DPEHPK3PXP&digits=9"
Error: Unsupported OTP parameter (digits=9). Only 6 to 8 digits are supported.
```

```sh
hermes-otp add --otpauth \
  "otpauth://totp/Ent:alice?secret=JBSWY3DPEHPK3PXP&issuer=Ent&algorithm=SHA256&digits=8&period=60"
```

### Export and Import

Export all records to a JSON backup file:
```sh
hermes-otp export -o backup.json
```

Import records from a JSON file:
```sh
hermes-otp import backup.json
```

The JSON format is a standard array of record objects:
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

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- SECURITY -->
## Security

### Independent record encryption

There is no vault key. Each record's secret is encrypted on its own, using the
password supplied when that record was created. Different records can use
different passwords: your `github` record can use one password and your `aws`
record another.

The upside is blast radius: if one record's password is compromised, the other
records remain secure. `update` proves access by decrypting that one record
before replacing it — it cannot verify a password against the whole file.

The trade-off is that `ls` can only decrypt the records whose password you give
it. Records encrypted with a different password are reported as failures, and
the command exits `3` when some decrypted and some did not.

### File permissions

On Unix the codex is kept owner-only: the file is `0600` (`-rw-------`), and the
app's own config directory is `0700` (`drwx------`). The same applies to the
`codex.bak` backup and to files written by `export`. Existing files with looser
permissions are repaired on the next write.

A directory you pass with `--path` is never re-permissioned, since it may be a
shared location such as `/tmp` or your home directory — only the codex file
itself is restricted there.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- Testing -->
## Testing

`cargo test`

### TOTP verification
- [https://authenticationtest.com/totpChallenge/](https://authenticationtest.com/totpChallenge/)
- [https://www.verifyr.com/en/otp/check#totp](https://www.verifyr.com/en/otp/check#totp)
- [https://totp.danhersam.com/](https://totp.danhersam.com/)

<!-- CONTRIBUTING -->
## Contributing

If you have a suggestion that would make this better, please fork the repo and
create a pull request. You can also simply open an issue with the tag
"enhancement".  Don't forget to give the project a star! Thanks again!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3. Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the Branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- LICENSE -->
## License

Distributed under the MIT License. See `LICENSE` for more information.

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTACT -->
## Contact

Just create an issue if you need something.

Project Link: [https://github.com/riccione/hermes-otp](https://github.com/riccione/hermes-otp)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ACKNOWLEDGMENTS -->
## Acknowledgments

* [Choose an Open Source License](https://choosealicense.com)
* [Rust](https://www.rust-lang.org/)

<p align="right">(<a href="#readme-top">back to top</a>)</p>
