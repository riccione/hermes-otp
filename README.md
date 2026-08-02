<a name="readme-top"></a>

<!-- PROJECT SHIELDS -->
<div align="center">

[![CI](https://github.com/riccione/hermes/actions/workflows/ci.yml/badge.svg)](https://github.com/riccione/hermes/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

</div>
<div align="center">
  <h3 align="center">Hermes: CLI OTP manager</h3>
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
        <li><a href="#pipeline-integration">Pipeline Integration</a></li>
        <li><a href="#automatically-copy-otp-code-to-clipboard">Clipboard</a></li>
      </ul>
    </li>
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

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- GETTING STARTED -->
## Getting Started

### Prerequisites

* Rust 1.85+

### Installation

Build from source:
`cargo build --release`

The binary is located in [target/release/hermes](https://github.com/riccione/hermes/releases).

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- USAGE EXAMPLES -->
## Usage

Running without arguments shows help.

There are 2 env variables:

* `HERMES_PASSWORD`: Password for codes.
* `HERMES_PATH`: Path to codex file.

Commands:

* `add -a <ALIAS> -c <CODE> [OPTIONS]`: Add new record.
* `remove -a <ALIAS> [OPTIONS]`: Remove record.
* `update -a <ALIAS> -c <CODE> [OPTIONS]`: Update code by alias.
* `rename <OLD ALIAS> <NEW ALIAS> [OPTIONS]`: Rename alias.
* `ls [OPTIONS]`: Get all OTP codes.
* `ls <ALIAS>`: Get OTP code by alias.
* `ls <PARTIAL MATCH>`: Get OTP codes by partial match. 
* `ls --exact <ALIAS>`: Get OTP codes by exact match. 
* `config`: Show location of the codex file.
* `migrate`: Migrate legacy codex format to JSON.

Flags:

* `-a`, `--alias`: Alias.
* `-c`, `--code`: Code aka Secret.
* `-p`, `--path`: Custom path to the codex file.
* `-u`, `--unencrypt`: WARNING: Store the secret in plain text. Use for debugging only.
* `--password`: WARNING: Using this flag leaves password in shell history.
* `-q`, `--quiet`: Only for `ls <ALIAS>`. Do not display progress bar.
* `-e`, `--exact`: Only for `ls <ALIAS`. Exact match.
* `-f [table, json]`, `--format [table, json]`: Only for `ls` command. Format output as table (default) or as JSON.

### Pipeline Integration

Hermes is designed for scripting and automation. Use `--format json` for machine-readable output and `-q` to suppress interactive prompts.

**Generate OTP in a script:**
```sh
OTP=$(hermes ls my_alias -q --format json | jq -r '.otp')
curl -X POST https://api.example.com/auth -d "otp=$OTP"
```

**Use with environment variables:**
```sh
export HERMES_PASSWORD="mysecret"
export HERMES_PATH="/secure/codex.json"
hermes ls my_alias -q
```

**Non-interactive mode (no progress bar):**
```sh
hermes ls my_alias -q | grep -oE '[0-9]{6}'
```

### Automatically copy OTP code to clipboard

**Wayland**
```sh
hermes ls my_alias | wl-copy
```

**X11**
```sh
hermes ls my_alias | xclip -selection clipboard
```

**macOS**
```sh
hermes ls my_alias | pbcopy
```

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

Project Link: [https://github.com/riccione/hermes](https://github.com/riccione/hermes)

<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- ACKNOWLEDGMENTS -->
## Acknowledgments

* [Choose an Open Source License](https://choosealicense.com)
* [Rust](https://www.rust-lang.org/)

<p align="right">(<a href="#readme-top">back to top</a>)</p>
