# Contributing to StarDash

Thanks for your interest in improving StarDash! Issues and pull requests may be written in
English or German.

## Reporting bugs and suggesting features

Please use the [issue forms](https://github.com/crazmoe/StarDash/issues/new/choose). For bugs,
include the StarDash version (`stardash --version` or the footer of the web interface), the
Linux distribution, the database (SQLite, MariaDB or PostgreSQL) and the versions of the clients
involved. Remove passwords, tokens and other personal data from logs and screenshots.

Problems in the clients themselves belong to
[StarCLX](https://github.com/crazmoe/StarCLX/issues), [StarCMX](https://github.com/crazmoe/StarCMX/issues)
or [StarMAC](https://github.com/crazmoe/StarMAC/issues).

Security problems are reported privately, see [SECURITY.md](SECURITY.md).

## Development setup

Requirements: Linux, Rust (stable). For the MariaDB and PostgreSQL tests a local database
server is needed; without one, those tests are skipped.

```sh
git clone https://github.com/crazmoe/StarDash.git
cd StarDash
cargo test --workspace
```

## Before opening a pull request

CI runs the following checks; please run them locally first:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- Keep a pull request focused on one change and describe what an administrator will notice.
- User-visible changes get a line in `CHANGELOG.md`.
- StarDash must never become a tool to monitor people: do not add data about users, presence,
  calls or working times, neither to reports, status messages, the dashboard nor the metrics.
- Do not commit credentials, server addresses or logs from real systems.

## License

By contributing, you agree that your contributions are licensed under the
[MIT License](../LICENSE).

## Code of Conduct

This project follows the [Contributor Covenant](CODE_OF_CONDUCT.md). By taking part you agree
to uphold it.
