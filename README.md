# StarDash

**English** · [Deutsch](README.de.md)

StarDash is the self-hosted support dashboard for the STARFACE clients
[StarCLX](https://github.com/crazmoe/StarCLX) (Linux), [StarCMX](https://github.com/crazmoe/StarCMX)
(Windows) and [StarMAC](https://github.com/crazmoe/StarMAC) (macOS). A company runs it on its own
Debian or Ubuntu server; the clients send error reports and a small status message to it, so IT
support can find problems without asking users for log files.

- **Off unless the company turns it on.** Clients only send anything when the administrator enters
  the StarDash address in the system-wide defaults file (`starclxrc` / `starcmxrc`). Users cannot
  enable or disable it themselves, and they see a notice that reports are collected.
- **Reports:** a "Send log to support" button, plus automatic reports after crashes, SIP failures,
  sign-in failures and failed reconnects after standby. Each report gets a short number such as
  `K7-2F9A` that users can read out on the phone.
- **Privacy by design:** clients scrub logs before sending them. External phone numbers are
  shortened and fingerprinted, names, e-mail addresses, chat texts and credentials are removed.
  Status messages contain the state of the program only: no users, presence, calls or call times.
- **Dashboard:** every workstation with its client version (outdated ones highlighted), connection
  state and recent errors; report viewer; create an issue with the log attached (GitHub, GitLab,
  Gitea/Forgejo).
- **Monitoring:** Prometheus metrics under `/metrics`, aggregated by default.
- **Easy to run:** a single Debian package with a systemd service. Data is stored in SQLite,
  MariaDB or PostgreSQL, chosen by the administrator. No Docker, no external services.

Not affiliated with STARFACE GmbH. "STARFACE" is used for descriptive purposes only.

> Works with StarCLX and StarCMX 2.0.0 and later. Web interface and commands in English and German.

## Installation

Download the Debian package for your architecture from the
[releases](https://github.com/crazmoe/StarDash/releases) and install it with
`sudo apt install ./stardash_<version>_amd64.deb`. Setup steps (company key,
web user, HTTPS reverse proxy, client defaults file, monitoring, privacy) are in
[packaging/doc/GUIDE.md](packaging/doc/GUIDE.md) (German:
[ANLEITUNG.md](packaging/doc/ANLEITUNG.md)), installed to
`/usr/share/doc/stardash/`.

## Contributing

Bug reports, ideas and pull requests are welcome, in English or German. See
[CONTRIBUTING.md](.github/CONTRIBUTING.md); security problems are reported privately as described
in [SECURITY.md](.github/SECURITY.md).

## License

[MIT](LICENSE)
