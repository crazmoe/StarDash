# Setting up StarDash

StarDash collects error reports and status messages from StarCLX (Linux),
StarCMX (Windows) and StarMAC (macOS) and shows them to IT support in a web
interface. StarDash runs on a Debian or Ubuntu server of the company. It sends
nothing to third parties, unless support deliberately creates an issue (see
[Issues](#8-issues)).

Deutsche Fassung: [ANLEITUNG.md](ANLEITUNG.md).

**Contents**

1. [Overview: who does what](#1-overview-who-does-what)
2. [Install](#2-install)
3. [Create a company](#3-create-a-company)
4. [Web interface users](#4-web-interface-users)
5. [HTTPS in front](#5-https-in-front)
6. [Turn on the clients](#6-turn-on-the-clients)
7. [Working with the web interface](#7-working-with-the-web-interface)
8. [Issues](#8-issues)
9. [Monitoring](#9-monitoring)
10. [Operation: update, backup, troubleshooting, removal](#10-operation)
11. [Privacy](#11-privacy) and [licence](#licence)
12. [All commands](#12-all-commands)

---

## 1. Overview: who does what

| Who | What | Where |
|---|---|---|
| **Server admin** (has `sudo` on the server) | Installs StarDash, creates companies and users, resets passwords, changes settings | Command line on the server |
| **Support** (role `support`) | Sees workstations and reports, reads logs, marks reports as done, writes notes, creates issues | Web interface |
| **Read-only** (role `read`) | Sees workstations and reports, reads logs; changes nothing and creates no issues | Web interface |
| **Client admin** (deploys StarCLX/StarCMX) | Adds address and key to the clients' policy file | Policy file on the workstations |
| **Staff** | Press "Send log to support" when something goes wrong and give support the report number | StarCLX/StarCMX |

There is deliberately no user management in the web interface: who may create
users or reset passwords is decided by access to the server (`sudo`). So there
is no admin password that someone could guess in a browser.

**Company:** every client sends a company key. StarDash uses it to assign
reports to a company. Most setups need exactly one company. An IT service
provider can create several customer companies and show each user only one of
them.

## 2. Install

Requirements: Debian 12/13 or Ubuntu 22.04/24.04, amd64 or arm64.

```sh
sudo apt install ./stardash_<version>_amd64.deb
```

During installation the package asks for the database:

- **sqlite** (default): needs nothing else and is enough for most companies.
  The file is `/var/lib/stardash/stardash.db`.
- **mariadb** or **postgresql**: useful when a database server with backups is
  already running. Create the database and user first and enter the address
  when asked for the connection:

```sh
# MariaDB
sudo mysql -e "CREATE DATABASE stardash; CREATE USER 'stardash'@'localhost' IDENTIFIED BY 'PASSWORD'; GRANT ALL ON stardash.* TO 'stardash'@'localhost';"
#   address: mysql://stardash:PASSWORD@localhost/stardash

# PostgreSQL
sudo -u postgres psql -c "CREATE USER stardash PASSWORD 'PASSWORD';" -c "CREATE DATABASE stardash OWNER stardash;"
#   address: postgres://stardash:PASSWORD@localhost/stardash
```

StarDash creates its tables on first start.

All settings are in **`/etc/stardash/config.toml`** (with an explanation of
every value). After a change:

```sh
sudo systemctl restart stardash
sudo stardash check      # checks settings, database and data folder
```

### Language

StarDash is available in English and German.

- **Web interface:** follows the browser language. "Deutsch · English" at the
  bottom of every page switches it; the browser remembers the choice.
- **Commands:** follow the server language (`LANG`, e.g. `en_GB.UTF-8`).
- **Fixed for everyone:** `language = "en"` or `language = "de"` in
  config.toml. The switch at the bottom of the page still works.

Every command has an English and a German name (`create-company` =
`firma-anlegen`, see [All commands](#12-all-commands)), and so has every key
and value in config.toml. The file uses the German names and shows the English
ones in brackets; you can write either:

```toml
language = "en"

[database]
type = "postgresql"
url = "postgres://stardash:PASSWORD@localhost/stardash"
reports = "files"        # or "database"

[issues]
mode = "default"         # "off", "own"
```

## 3. Create a company

```sh
sudo stardash create-company "Example Ltd"
```

The output shows the **key** for the clients:

```
Company: Example Ltd
Key: 8fK2…

Add to the clients' policy file (starclxrc / starcmxrc):

[Support]
url=https://<address of StarDash>
token=8fK2…
```

Write the key down right away: StarDash stores it only as a hash and cannot
show it again.

- **Key lost or leaked?** Generate a new one:
  `sudo stardash company-key "Example Ltd"`. The old one stops working
  immediately; the clients' policy file needs the new one. Until then their
  reports wait on the workstations and are sent later (at most 20 reports per
  workstation).
- **Which companies exist?** `sudo stardash companies`

## 4. Web interface users

### Roles

| | `support` | `read` |
|---|:---:|:---:|
| View overview, workstations, reports | ✓ | ✓ |
| Read and download logs | ✓ | ✓ |
| Mark a report as done, write a note | ✓ | – |
| Create an issue | ✓ | – |

`read` suits, for example, a help desk that only needs to look things up, or
trainees.

Each user can also be limited to **one company**. Without one, the user sees
all companies.

### Create

```sh
sudo stardash user anna                          # role support, all companies
sudo stardash user helpdesk read                 # read-only, all companies
sudo stardash user partner support "Example Ltd" # only Example Ltd
```

StarDash generates the password itself and shows it once:

```
User:     anna
Password: Xp4…
Role:     support
```

Passwords cannot be chosen and cannot be changed in the web interface. This is
on purpose: they are random and long enough that nobody guesses them. Best
keep them in a password manager.

### Reset a password, change role or company

Run the same command with the same name again:

```sh
sudo stardash user anna                # new password, role support
sudo stardash user anna read           # new password, now read-only
```

This always creates a new password and signs the user out in all open
browsers. **Note:** role and company are set anew each time. If the user had
`read` or a company, give both again, otherwise `support` for all companies
applies.

### List and delete

```sh
sudo stardash users                   # name, role, company
sudo stardash delete-user anna        # deletes and signs out immediately
```

### Signing in

- A sign-in lasts 12 hours, then StarDash asks again.
- "Sign out" at the top right ends it immediately.
- Failed sign-ins are in the system log (`journalctl -u stardash`) with the
  name entered, never with the password. Against guessing, add a limit in the
  web server in front (e.g. fail2ban on the log or `limit_req` in nginx).

## 5. HTTPS in front

StarDash listens only on `127.0.0.1:8470` and so cannot be reached from
outside. Put a web server with a valid certificate in front; the workstations
and support connect to it. Ready-made examples are in
`/usr/share/doc/stardash/`:

- `nginx.conf`
- `apache.conf`
- `Caddyfile` (gets the certificate itself)

Example with nginx:

```sh
sudo apt install nginx
sudo cp /usr/share/doc/stardash/nginx.conf /etc/nginx/sites-available/stardash
sudo nano /etc/nginx/sites-available/stardash   # set server name and certificate
sudo ln -s ../sites-available/stardash /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

Test: `https://stardash.example.com/api/v1/health` shows `ok`.

The clients verify the certificate. A self-signed certificate only works if it
is installed as trusted on the workstations (e.g. the company's internal
certificate authority).

## 6. Turn on the clients

The clients only send when the company enables it in the **policy file**.
Staff can neither turn it on nor off.

| Client | Policy file |
|---|---|
| StarCLX (Linux) | `/etc/xdg/starclxrc` |
| StarCMX (Windows) | `C:\ProgramData\StarCMX\starcmxrc` |

At least:

```ini
[Support]
url=https://stardash.example.com
token=<key from step 3>
```

All values of the `[Support]` group:

| Value | Meaning | Default |
|---|---|---|
| `url` | Address of StarDash (with `https://`) | – (without `url` the client never sends anything) |
| `token` | Company key from `create-company` | – |
| `auto` | When the client sends a report by itself, comma separated: `crash`, `sip` (softphone unregistered or SIP error), `login` (sign-in to the PBX fails), `wake` (no connection after standby). `off` (or `none`) = only by button | all four |
| `verbose` | Detailed log always on, so reports have enough detail (`true`/`false`, also `yes`/`on`/`1`) | `true` |
| `heartbeat` | Minutes between status messages, `0` = none | `15` |
| `internal_numbers` | `hidden` also hides internal numbers (extensions) | readable |

Example with all values:

```ini
[Support]
url=https://stardash.example.com
token=8fK2…
auto=crash,sip
verbose=true
heartbeat=15
internal_numbers=hidden
```

Values work in English and German. The clients read the file **at start**.
After a change, restart StarCLX/StarCMX (or sign out and in again).

On the first start with `[Support]` set, staff see a notice once that error
logs go to IT support. In the settings under **Log** there is the button
**"Send log to support"** with a field "What happened?". After sending, the
client shows the **report number** (e.g. `K7-2F9A`), easy to read out on the
phone.

Automatic reports are limited: at most one per hour and reason, at most ten a
day. If StarDash cannot be reached, the report waits on the workstation and is
sent later.

## 7. Working with the web interface

Open `https://stardash.example.com` and sign in.

- **Overview:** number of workstations, how many currently have problems or
  are unreachable, errors of the last days by version, latest reports.
- **Workstations:** every computer with app, version (outdated versions are
  marked; StarDash checks GitHub for the latest version at start and every 6 hours, `check_releases`), operating system, connection to the PBX, chat, softphone, audio and
  last message.
- **State of a workstation:**

  | State | Meaning |
  |---|---|
  | **OK** (green) | The last status message shows: connected to the PBX, softphone registered (if set up), audio OK, and no open report from the last 24 hours |
  | **Error** (red) | Something of that is missing, or there is an open report from the last 24 hours |
  | **Unreachable** | No status message for 45 minutes (`offline_after_minutes`) or the app was closed; usually the computer is simply off |
  | **Reports only** | The client sends no status messages (`heartbeat=0`), so no state |

  A workstation turns green once all open reports are marked **done** (or are
  older than 24 hours) and the last status message is OK. Status messages come
  every 15 minutes and immediately when something changes on the client (e.g.
  softphone registered again).
- **Reports:** searchable by report number, computer name, reason and
  version. A report shows the workstation details, the text the staff member
  wrote and the cleaned log (errors highlighted, downloadable).
- **Done and note** (role `support`): lets the team see what has already been
  looked at. Done reports no longer count as an error of the workstation and
  disappear from the "open only" list.

Typical flow: a staff member calls and says `K7-2F9A`. Support searches the
number under **Reports**, reads the log and solves the problem or creates an
issue for the developers.

## 8. Issues

Below a report there is (role `support`) the **"Create issue"** button.
StarDash first shows a **preview** of title, text and log, which support can
edit. A tick box there also hides internal numbers.

Where the issue goes is set in `/etc/stardash/config.toml` under `[issues]`:

| `mode` | Target |
|---|---|
| `default` (`vorgabe`) | Public issue with the developers on github.com (StarCLX or StarCMX repository). **Publicly readable:** check the preview carefully. |
| `off` (`aus`) | No "Create issue" button |
| `own` (`eigene`) | Only your own targets: GitHub, GitLab, Gitea or Forgejo, e.g. an internal ticket project |

With `default` and no `github_token`, StarDash opens a prefilled issue in the
browser; support signs in there with their own GitHub account and attaches the
log. With `github_token`, StarDash creates the issue and the log (as a gist)
itself.

Own target, GitLab example:

```toml
[issues]
mode = "own"

[[issues.target]]
name = "IT telephony"
type = "gitlab"             # github, gitlab, gitea (also Forgejo)
url = "https://gitlab.example.com"
project = "it/telephony"    # GitHub/Gitea: owner/repo
token = "glpat-…"           # permission: create issues
apps = []                   # empty = all, otherwise e.g. ["StarCMX"]
```

## 9. Monitoring

`https://stardash.example.com/metrics` provides figures in Prometheus format
(readable by Prometheus, Grafana, Zabbix, Checkmk, Icinga):

- workstations per app, version and state (ok, error, unreachable)
- registered softphones, outdated versions
- errors today by kind, stored reports per reason

By default only **totals**. `metrics_per_host = true` also outputs the state of
each computer (meant for outages, not for checking attendance).

Without `metrics_token`, `/metrics` is readable without authentication. Then
restrict the path to the internal network in the web server, or set a token.

**Create a token:**

1. Generate a random token (any long text, e.g. 64 characters):

   ```sh
   openssl rand -hex 32
   ```

2. Put the result into `/etc/stardash/config.toml`:

   ```toml
   metrics_token = "3f9c…(output of openssl)…"
   ```

3. Restart the service: `sudo systemctl restart stardash`

4. Check: without the token `/metrics` now answers `401`, with it the values.

   ```sh
   curl -i https://stardash.example.com/metrics
   curl -H "Authorization: Bearer 3f9c…" https://stardash.example.com/metrics
   ```

**Add it to the monitoring:** the monitoring sends the token in the header
`Authorization: Bearer <token>`. Prometheus example:

```yaml
scrape_configs:
  - job_name: stardash
    scheme: https
    metrics_path: /metrics
    authorization:
      type: Bearer
      credentials: "3f9c…"
    static_configs:
      - targets: ["stardash.example.com"]
```

Zabbix (HTTP agent), Checkmk and Icinga have a field for custom headers:
`Authorization` with the value `Bearer 3f9c…`.

**Change the token:** generate a new one, replace it in `config.toml`, restart
the service and update the monitoring. The old token stops working. The token
only covers `/metrics`, not the clients (they use the company key) and not the
login to the web interface.

## 10. Operation

**Update:** install the new package; settings are kept:

```sh
sudo apt install ./stardash_<new-version>_amd64.deb
```

**Backup:**

- `/etc/stardash/config.toml`
- SQLite: `/var/lib/stardash/` (database and reports), ideally with the
  service stopped or via `sqlite3 stardash.db ".backup backup.db"`
- MariaDB/PostgreSQL: back up the database as usual; if reports are stored as
  files (`reports = "files"`), also `/var/lib/stardash/`

**Troubleshooting:**

```sh
systemctl status stardash
journalctl -u stardash -e          # service log
sudo stardash check
curl -s http://127.0.0.1:8470/api/v1/health
```

No reports arriving? Is `url` in the policy file correct, was the client
restarted after the change, can the workstation reach StarDash
(`/api/v1/health` in the workstation's browser)? A wrong key shows up in the
service log as a rejected request.

**Retention:** reports are deleted after `retention_days` (default 30 days),
workstations without messages after the same time.

**Removal:**

```sh
sudo apt remove stardash    # program gone, data and settings stay
sudo apt purge stardash     # also deletes /var/lib/stardash and settings
```

`purge` does not delete a MariaDB or PostgreSQL database.

## 11. Privacy

- The clients hide external phone numbers, names, e-mail addresses, chat
  texts and credentials **before** anything leaves the computer. Internal
  numbers stay readable unless `internal_numbers=hidden`.
- Reports show the **computer name** instead of the user.
- StarDash stores no users, no presence, no calls and no call times. It cannot
  show when or how much someone works; do not use the data for that either
  (Switzerland: ArGV 3 Art. 26; similar rules apply in other countries).
- Inform staff, e.g. in the IT policy: "The phone app sends technical error
  logs to IT support (company server). Phone numbers and names in them are
  hidden; call content is never sent."
- Issues with `mode = "default"` are public. Check the preview before
  submitting, or choose `mode = "off"` or `"own"`.

## Licence

StarDash is MIT licensed. The licences of all built-in third-party components
are in `/usr/share/doc/stardash/THIRD-PARTY-LICENSES.txt` and in the web
interface under **Licences** (bottom of every page). StarDash is not a product
of STARFACE GmbH; "STARFACE" is used for description only.

## 12. All commands

`stardash [--config FILE] COMMAND`

| Command | German | What |
|---|---|---|
| `serve` | `serve` | Run the service (done by systemd) |
| `create-company NAME` | `firma-anlegen` | Create a company, shows the key for the clients |
| `company-key NAME` | `firma-schluessel` | Generate a new key, the old one stops working |
| `companies` | `firmen` | List companies |
| `user NAME [ROLE] [COMPANY]` | `benutzer` | Create a user or reset the password (ROLE: `support` or `read`, default `support`; COMPANY: name, default all companies) |
| `users` | `benutzer-liste` | List users |
| `delete-user NAME` | `benutzer-loeschen` | Delete a user and sign them out |
| `check` | `pruefen` | Check settings, database and data folder |
| `help` | `hilfe` | List of commands |
| `--version` | `--version` | Show the version |

Run commands that change companies or users with `sudo`.
