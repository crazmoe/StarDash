# StarDash einrichten

StarDash sammelt Fehlerberichte und Statusmeldungen von StarCLX (Linux),
StarCMX (Windows) und StarMAC (macOS) und zeigt sie dem IT-Support in einer
Weboberfläche. StarDash läuft auf einem Debian- oder Ubuntu-Server der Firma.
Es schickt nichts an Dritte, ausser der Support erstellt bewusst ein Issue
(siehe [Issues](#8-issues)).

English version: [GUIDE.md](GUIDE.md).

**Inhalt**

1. [Überblick: wer macht was](#1-überblick-wer-macht-was)
2. [Installieren](#2-installieren)
3. [Firma anlegen](#3-firma-anlegen)
4. [Benutzer der Weboberfläche](#4-benutzer-der-weboberfläche)
5. [HTTPS davor](#5-https-davor)
6. [Clients einschalten](#6-clients-einschalten)
7. [Arbeiten mit der Weboberfläche](#7-arbeiten-mit-der-weboberfläche)
8. [Issues](#8-issues)
9. [Monitoring](#9-monitoring)
10. [Betrieb: Update, Sicherung, Fehlersuche, Entfernen](#10-betrieb)
11. [Datenschutz](#11-datenschutz) und [Lizenz](#lizenz)
12. [Alle Befehle](#12-alle-befehle)

---

## 1. Überblick: wer macht was

| Wer | Was | Wo |
|---|---|---|
| **Server-Admin** (hat `sudo` auf dem Server) | Installiert StarDash, legt Firmen und Benutzer an, setzt Passwörter zurück, ändert Einstellungen | Kommandozeile auf dem Server |
| **Support** (Rolle `support`) | Sieht Arbeitsplätze und Berichte, liest Protokolle, hakt Berichte als erledigt ab, schreibt Notizen, erstellt Issues | Weboberfläche |
| **Nur lesen** (Rolle `lesen`) | Sieht Arbeitsplätze und Berichte, liest Protokolle; ändert nichts und erstellt keine Issues | Weboberfläche |
| **Client-Admin** (verteilt StarCLX/StarCMX) | Trägt Adresse und Schlüssel in die Vorgabedatei der Clients ein | Vorgabedatei auf den Arbeitsplätzen |
| **Mitarbeitende** | Drücken bei Problemen „Protokoll an Support senden“ und nennen dem Support die Bericht-Nr. | StarCLX/StarCMX |

Eine Verwaltung der Benutzer in der Weboberfläche gibt es bewusst nicht: Wer
Benutzer anlegen oder Passwörter zurücksetzen darf, entscheidet der Zugang zum
Server (`sudo`). So gibt es kein Admin-Passwort, das jemand im Browser erraten
könnte.

**Firma:** Jeder Client schickt einen Firmen-Schlüssel mit. Darüber ordnet
StarDash Berichte einer Firma zu. Die meisten brauchen genau eine Firma. Ein
IT-Dienstleister kann mehrere Kundenfirmen anlegen und jedem Benutzer nur eine
davon zeigen.

## 2. Installieren

Voraussetzung: Debian 12/13 oder Ubuntu 22.04/24.04, amd64 oder arm64.

```sh
sudo apt install ./stardash_<version>_amd64.deb
```

Bei der Installation fragt das Paket nach der Datenbank:

- **sqlite** (Standard): braucht nichts weiter und reicht für die meisten
  Firmen. Die Datei liegt unter `/var/lib/stardash/stardash.db`.
- **mariadb** oder **postgresql**: sinnvoll, wenn schon ein Datenbankserver mit
  Sicherung läuft. Datenbank und Benutzer vorher anlegen und bei der Frage nach
  der Verbindung die Adresse eingeben:

```sh
# MariaDB
sudo mysql -e "CREATE DATABASE stardash; CREATE USER 'stardash'@'localhost' IDENTIFIED BY 'PASSWORT'; GRANT ALL ON stardash.* TO 'stardash'@'localhost';"
#   Adresse: mysql://stardash:PASSWORT@localhost/stardash

# PostgreSQL
sudo -u postgres psql -c "CREATE USER stardash PASSWORD 'PASSWORT';" -c "CREATE DATABASE stardash OWNER stardash;"
#   Adresse: postgres://stardash:PASSWORT@localhost/stardash
```

Die Tabellen legt StarDash beim ersten Start selbst an.

Alle Einstellungen stehen in **`/etc/stardash/config.toml`** (mit Erklärungen
zu jedem Wert). Nach einer Änderung:

```sh
sudo systemctl restart stardash
sudo stardash pruefen      # prüft Einstellungen, Datenbank und Datenordner
```

### Sprache

StarDash gibt es auf Deutsch und Englisch.

- **Weboberfläche:** folgt der Sprache des Browsers. Unten auf jeder Seite
  lässt sie sich mit „Deutsch · English“ umstellen; der Browser merkt sich die
  Wahl.
- **Befehle:** folgen der Sprache des Servers (`LANG`, z. B. `de_CH.UTF-8`).
- **Fest einstellen** für alle: `language = "de"` oder `language = "en"` in
  der config.toml. Die Umschaltung unten auf der Seite geht trotzdem.

Jeder Befehl hat auch einen englischen Namen (`firma-anlegen` =
`create-company`, siehe [Alle Befehle](#12-alle-befehle)), ebenso jeder Name
und Wert in der config.toml (`[datenbank]` = `[database]`, `modus = "aus"` =
`mode = "off"`; die Datei nennt sie in Klammern).

## 3. Firma anlegen

```sh
sudo stardash firma-anlegen "Muster AG"
```

Die Ausgabe zeigt den **Schlüssel** für die Clients:

```
Firma: Muster AG
Schlüssel: 8fK2…

In die Vorgabedatei der Clients (starclxrc / starcmxrc):

[Support]
url=https://<Adresse von StarDash>
token=8fK2…
```

Den Schlüssel sofort notieren: StarDash speichert ihn nur verschlüsselt
(gehasht) und kann ihn nicht noch einmal anzeigen.

- **Schlüssel verloren oder weitergegeben?** Neuen erzeugen:
  `sudo stardash firma-schluessel "Muster AG"`. Der alte gilt danach sofort
  nicht mehr; die Vorgabedatei der Clients muss den neuen bekommen. Bis dahin
  bleiben deren Berichte auf den Arbeitsplätzen liegen und werden später
  nachgeschickt (höchstens 20 Berichte je Arbeitsplatz).
- **Welche Firmen gibt es?** `sudo stardash firmen`

## 4. Benutzer der Weboberfläche

### Rollen

| | `support` | `lesen` |
|---|:---:|:---:|
| Übersicht, Arbeitsplätze, Berichte ansehen | ✓ | ✓ |
| Protokolle lesen und herunterladen | ✓ | ✓ |
| Bericht als erledigt markieren, Notiz schreiben | ✓ | – |
| Issue erstellen | ✓ | – |

`lesen` passt z. B. für eine Hotline, die nur nachschauen soll, oder für
Lernende.

Zusätzlich lässt sich jeder Benutzer auf **eine Firma** beschränken. Ohne
Angabe sieht er alle Firmen.

### Anlegen

```sh
sudo stardash benutzer anna                       # Rolle support, alle Firmen
sudo stardash benutzer hotline lesen              # nur lesen, alle Firmen
sudo stardash benutzer partner support "Muster AG"  # nur Firma Muster AG
```

StarDash erzeugt das Passwort selbst und zeigt es einmal an:

```
Benutzer: anna
Passwort: Xp4…
Rolle:    support
```

Passwörter lassen sich nicht selbst wählen und nicht in der Weboberfläche
ändern. Das ist Absicht: Sie sind zufällig und lang genug, dass sie niemand
errät. Am besten im Passwort-Manager speichern.

### Passwort zurücksetzen, Rolle oder Firma ändern

Denselben Befehl mit demselben Namen noch einmal ausführen:

```sh
sudo stardash benutzer anna                 # neues Passwort, Rolle support
sudo stardash benutzer anna lesen           # neues Passwort, jetzt nur lesen
```

Dabei entsteht immer ein neues Passwort, und der Benutzer wird in allen offenen
Browsern abgemeldet. **Achtung:** Rolle und Firma werden jedes Mal neu gesetzt.
Wer `lesen` oder eine Firma hatte, muss beides wieder angeben, sonst gilt
`support` für alle Firmen.

### Auflisten und löschen

```sh
sudo stardash benutzer-liste            # Name, Rolle, Firma
sudo stardash benutzer-loeschen anna    # löscht und meldet sofort ab
```

### Anmeldung

- Eine Anmeldung gilt 12 Stunden, danach fragt StarDash erneut.
- „Abmelden“ oben rechts beendet sie sofort.
- Fehlgeschlagene Anmeldungen stehen im Systemprotokoll
  (`journalctl -u stardash`), mit dem eingegebenen Namen, nie mit Passwort.
  Gegen Rateversuche hilft eine Sperre im Webserver davor (z. B. fail2ban auf
  das Protokoll oder `limit_req` in nginx).

## 5. HTTPS davor

StarDash lauscht nur auf `127.0.0.1:8470` und ist so von aussen nicht
erreichbar. Davor gehört ein Webserver mit gültigem Zertifikat, auf den die
Arbeitsplätze und der Support zugreifen. Fertige Beispiele liegen unter
`/usr/share/doc/stardash/`:

- `nginx.conf`
- `apache.conf`
- `Caddyfile` (holt das Zertifikat selbst)

Beispiel mit nginx:

```sh
sudo apt install nginx
sudo cp /usr/share/doc/stardash/nginx.conf /etc/nginx/sites-available/stardash
sudo nano /etc/nginx/sites-available/stardash   # Servername und Zertifikat eintragen
sudo ln -s ../sites-available/stardash /etc/nginx/sites-enabled/
sudo nginx -t && sudo systemctl reload nginx
```

Test: `https://stardash.firma.ch/api/v1/health` zeigt `ok`.

Die Clients prüfen das Zertifikat. Ein selbst signiertes Zertifikat geht nur,
wenn es auf den Arbeitsplätzen als vertrauenswürdig installiert ist (z. B. die
interne Zertifizierungsstelle der Firma).

## 6. Clients einschalten

Die Clients senden nur, wenn die Firma es in der **Vorgabedatei** einträgt.
Mitarbeitende können es weder ein- noch ausschalten.

| Client | Vorgabedatei |
|---|---|
| StarCLX (Linux) | `/etc/xdg/starclxrc` |
| StarCMX (Windows) | `C:\ProgramData\StarCMX\starcmxrc` |

Mindestens:

```ini
[Support]
url=https://stardash.firma.ch
token=<Schlüssel aus Schritt 3>
```

Alle Werte der Gruppe `[Support]`:

| Wert | Bedeutung | Standard |
|---|---|---|
| `url` | Adresse von StarDash (mit `https://`) | – (ohne `url` sendet der Client nie etwas) |
| `token` | Firmen-Schlüssel aus `firma-anlegen` | – |
| `auto` | Wann der Client von selbst einen Bericht schickt, durch Komma getrennt: `crash` (Absturz), `sip` (Softphone abgemeldet oder SIP-Fehler), `login` (Anmeldung an der Anlage scheitert), `wake` (nach dem Standby keine Verbindung). `aus` (oder `off`, `none`) = nur per Knopf | alle vier |
| `verbose` | Ausführliches Protokoll immer an, damit Berichte genug Details haben (`true`/`false`, auch `ja`/`yes`/`on`/`1`) | `true` |
| `heartbeat` | Minuten zwischen Statusmeldungen, `0` = keine | `15` |
| `internal_numbers` | `unkenntlich` (oder `hidden`) macht auch interne Nummern (Durchwahlen) unkenntlich | lesbar |

Beispiel mit allen Werten:

```ini
[Support]
url=https://stardash.firma.ch
token=8fK2…
auto=crash,sip
verbose=true
heartbeat=15
internal_numbers=unkenntlich
```

Die Werte gehen auf Deutsch und Englisch. Die Clients lesen die Datei **beim Start**. Nach einer Änderung StarCLX/StarCMX
neu starten (oder abmelden und wieder anmelden).

Beim ersten Start mit eingetragenem `[Support]` sehen die Mitarbeitenden einmal
einen Hinweis, dass Fehlerprotokolle an den IT-Support gehen. In den
Einstellungen unter **Protokoll** gibt es den Knopf **„Protokoll an Support
senden“** mit einem Feld „Was ist passiert?“. Nach dem Senden zeigt der Client
die **Bericht-Nr.** (z. B. `K7-2F9A`), die sich gut am Telefon durchgeben lässt.

Automatische Berichte sind begrenzt: höchstens einer pro Stunde und Anlass,
höchstens zehn am Tag. Ist StarDash nicht erreichbar, wartet der Bericht auf
dem Arbeitsplatz und geht später raus.

## 7. Arbeiten mit der Weboberfläche

`https://stardash.firma.ch` öffnen und anmelden.

- **Übersicht:** Anzahl Arbeitsplätze, wie viele gerade Probleme haben oder
  nicht erreichbar sind, Fehler der letzten Tage nach Version, neueste Berichte.
- **Arbeitsplätze:** jeder Rechner mit Programm, Version (veraltete Versionen
  sind markiert), Betriebssystem, Verbindung zur Anlage, Chat, Softphone, Ton
  und letzter Meldung.
- **Zustand eines Arbeitsplatzes:**

  | Zustand | Bedeutung |
  |---|---|
  | **In Ordnung** (grün) | Letzte Statusmeldung zeigt: mit der Anlage verbunden, Softphone angemeldet (falls eingerichtet), Ton in Ordnung, und kein offener Bericht aus den letzten 24 Stunden |
  | **Fehler** (rot) | Etwas davon fehlt, oder es gibt einen offenen Bericht aus den letzten 24 Stunden |
  | **Nicht erreichbar** | Seit 45 Minuten keine Statusmeldung (`offline_after_minutes`) oder das Programm wurde beendet; meist ist der Rechner einfach aus |
  | **Nur Berichte** | Der Client schickt keine Statusmeldungen (`heartbeat=0`), daher kein Zustand |

  Ein Arbeitsplatz wird grün, sobald alle offenen Berichte als **erledigt**
  markiert sind (oder älter als 24 Stunden) und die letzte Statusmeldung in
  Ordnung ist. Die Statusmeldung kommt alle 15 Minuten und sofort, wenn sich
  am Client etwas ändert (z. B. Softphone wieder angemeldet).
- **Berichte:** suchbar nach Bericht-Nr., Rechnername, Anlass und Version.
  Ein Bericht zeigt die Angaben zum Arbeitsplatz, den Text der Mitarbeiterin
  oder des Mitarbeiters und das bereinigte Protokoll (Fehler hervorgehoben,
  herunterladbar).
- **Erledigt und Notiz** (Rolle `support`): Damit sieht das Team, was schon
  angeschaut wurde. Erledigte Berichte zählen nicht mehr als Fehler des
  Arbeitsplatzes und verschwinden aus der Liste „nur offene“.

Typischer Ablauf: Eine Mitarbeiterin ruft an und nennt `K7-2F9A`. Der Support
sucht die Nummer unter **Berichte**, liest das Protokoll und löst das Problem
oder erstellt ein Issue für die Entwickler.

## 8. Issues

Unter einem Bericht gibt es (Rolle `support`) den Knopf **„Issue erstellen“**.
StarDash zeigt vorher eine **Vorschau** mit Titel, Text und Protokoll, die der
Support anpassen kann. Interne Nummern lassen sich dort mit einem Häkchen
zusätzlich unkenntlich machen.

Wohin das Issue geht, steht in `/etc/stardash/config.toml` unter `[issues]`:

| `modus` | Ziel |
|---|---|
| `vorgabe` (Standard) | Öffentliches Issue bei den Entwicklern auf github.com (Repository von StarCLX bzw. StarCMX). **Öffentlich lesbar:** Vorschau genau prüfen. |
| `aus` | Kein Knopf „Issue erstellen“ |
| `eigene` | Nur eigene Ziele: GitHub, GitLab, Gitea oder Forgejo, z. B. ein internes Ticket-Projekt |

Bei `vorgabe` ohne `github_token` öffnet StarDash ein vorausgefülltes Issue im
Browser; der Support meldet sich dort mit seinem eigenen GitHub-Konto an und
fügt das Protokoll ein. Mit `github_token` legt StarDash Issue und Protokoll
(als Gist) selbst an.

Eigenes Ziel, Beispiel GitLab:

```toml
[issues]
modus = "eigene"

[[issues.ziel]]
name = "IT Telefonie"
art = "gitlab"              # github, gitlab, gitea (auch Forgejo)
url = "https://gitlab.firma.ch"
projekt = "it/telefonie"    # GitHub/Gitea: besitzer/repo
token = "glpat-…"           # Recht: Issues anlegen
apps = []                   # leer = alle, sonst z. B. ["StarCMX"]
```

## 9. Monitoring

`https://stardash.firma.ch/metrics` liefert Kennzahlen im Prometheus-Format
(lesbar für Prometheus, Grafana, Zabbix, Checkmk, Icinga):

- Arbeitsplätze je Programm, Version und Zustand (ok, Fehler, nicht erreichbar)
- angemeldete Softphones, veraltete Versionen
- Fehler heute nach Art, gespeicherte Berichte je Anlass

Standardmässig nur **zusammengezählt**. `metrics_per_host = true` gibt
zusätzlich den Zustand einzelner Rechner aus (für Störungen gedacht, nicht um
Anwesenheit zu prüfen).

Ohne `metrics_token` ist `/metrics` ohne Anmeldung lesbar. Dann den Pfad im
Webserver auf das interne Netz beschränken oder ein Token setzen:

```toml
metrics_token = "langes-zufälliges-token"
```

Das Monitoring sendet es als `Authorization: Bearer langes-zufälliges-token`.

## 10. Betrieb

**Update:** neues Paket installieren, die Einstellungen bleiben erhalten:

```sh
sudo apt install ./stardash_<neue-version>_amd64.deb
```

**Sicherung:**

- `/etc/stardash/config.toml`
- SQLite: `/var/lib/stardash/` (Datenbank und Berichte), am besten mit
  angehaltenem Dienst oder per `sqlite3 stardash.db ".backup sicherung.db"`
- MariaDB/PostgreSQL: die Datenbank wie gewohnt sichern; liegen die Berichte
  als Dateien (`berichte = "dateien"`), zusätzlich `/var/lib/stardash/`

**Fehlersuche:**

```sh
systemctl status stardash
journalctl -u stardash -e          # Protokoll des Dienstes
sudo stardash pruefen
curl -s http://127.0.0.1:8470/api/v1/health
```

Kommen keine Berichte an: Stimmt `url` in der Vorgabedatei, ist der Client
nach der Änderung neu gestartet, ist StarDash vom Arbeitsplatz aus erreichbar
(`/api/v1/health` im Browser des Arbeitsplatzes)? Ein falscher Schlüssel
erscheint im Protokoll des Dienstes als abgelehnte Anfrage.

**Aufbewahrung:** Berichte werden nach `retention_days` (Standard 30 Tage)
gelöscht, Arbeitsplätze ohne Meldung nach derselben Zeit.

**Entfernen:**

```sh
sudo apt remove stardash    # Programm weg, Daten und Einstellungen bleiben
sudo apt purge stardash     # zusätzlich /var/lib/stardash und Einstellungen löschen
```

Eine MariaDB- oder PostgreSQL-Datenbank löscht `purge` nicht.

## 11. Datenschutz

- Die Clients machen externe Telefonnummern, Namen, E-Mail-Adressen,
  Chat-Texte und Zugangsdaten unkenntlich, **bevor** etwas den Rechner
  verlässt. Interne Nummern bleiben lesbar, ausser
  `internal_numbers=unkenntlich`.
- Statt des Benutzers steht der **Rechnername** im Bericht.
- StarDash speichert keine Benutzer, keine Präsenz, keine Gespräche und keine
  Gesprächszeiten. Daraus lässt sich nicht ablesen, wann oder wie viel jemand
  arbeitet; die Daten auch nicht dafür verwenden (Schweiz: ArGV 3 Art. 26).
- Mitarbeitende informieren, z. B. im IT-Reglement: „Die Telefon-App sendet
  technische Fehlerprotokolle an den IT-Support (Server der Firma).
  Telefonnummern und Namen sind darin unkenntlich gemacht; Gesprächsinhalte
  werden nie gesendet.“
- Issues mit `modus = "vorgabe"` sind öffentlich. Vor dem Absenden die Vorschau
  prüfen, oder `modus = "aus"` bzw. `"eigene"` wählen.

## Lizenz

StarDash steht unter der MIT-Lizenz. Die Lizenzen aller eingebauten
Fremdkomponenten stehen in `/usr/share/doc/stardash/THIRD-PARTY-LICENSES.txt`
und in der Weboberfläche unter **Lizenzen** (unten auf jeder Seite). StarDash
ist kein Produkt der STARFACE GmbH; „STARFACE“ wird nur beschreibend verwendet.

## 12. Alle Befehle

`stardash [--config DATEI] BEFEHL`

| Befehl | Englisch | Was |
|---|---|---|
| `serve` | `serve` | Dienst starten (macht systemd) |
| `firma-anlegen NAME` | `create-company` | Firma anlegen, zeigt den Schlüssel für die Clients |
| `firma-schluessel NAME` | `company-key` | Neuen Schlüssel erzeugen, der alte gilt nicht mehr |
| `firmen` | `companies` | Firmen auflisten |
| `benutzer NAME [ROLLE] [FIRMA]` | `user` | Benutzer anlegen oder Passwort zurücksetzen (ROLLE: `support` oder `lesen`/`read`, Standard `support`; FIRMA: Name, Standard alle Firmen) |
| `benutzer-liste` | `users` | Benutzer auflisten |
| `benutzer-loeschen NAME` | `delete-user` | Benutzer löschen und abmelden |
| `pruefen` | `check` | Einstellungen, Datenbank und Datenordner prüfen |
| `hilfe` | `help` | Übersicht der Befehle |
| `--version` | `--version` | Version zeigen |

Befehle, die Firmen oder Benutzer ändern, mit `sudo` ausführen.
