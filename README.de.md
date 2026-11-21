# StarDash

[English](README.md) · **Deutsch**

StarDash ist die selbst gehostete Support-Übersicht für die STARFACE-Clients
[StarCLX](https://github.com/crazmoe/StarCLX) (Linux), [StarCMX](https://github.com/crazmoe/StarCMX)
(Windows) und [StarMAC](https://github.com/crazmoe/StarMAC) (macOS). Eine Firma betreibt sie auf
ihrem eigenen Debian- oder Ubuntu-Server. Die Clients schicken Fehlerberichte und eine kleine
Statusmeldung dorthin, damit der IT-Support Probleme findet, ohne Benutzer um Protokolldateien
bitten zu müssen.

- **Aus, solange die Firma es nicht einschaltet.** Die Clients senden nur, wenn der Admin die
  Adresse von StarDash in der Vorgabedatei (`starclxrc` / `starcmxrc`) einträgt. Benutzer können
  es weder ein- noch ausschalten und sehen einen Hinweis, dass gesammelt wird.
- **Berichte:** Knopf „Protokoll an Support senden“, dazu automatische Berichte nach Abstürzen,
  SIP-Fehlern, gescheiterter Anmeldung und fehlender Verbindung nach dem Standby. Jeder Bericht
  bekommt eine kurze Nummer wie `K7-2F9A`, die man am Telefon durchgeben kann.
- **Datenschutz von Anfang an:** Die Clients bereinigen das Protokoll vor dem Senden. Externe
  Nummern werden gekürzt und mit einem Fingerabdruck ersetzt; Namen, E-Mail-Adressen, Chat-Texte
  und Zugangsdaten fallen weg. Statusmeldungen enthalten nur den Zustand des Programms: keine
  Benutzer, keine Präsenz, keine Gespräche und keine Gesprächszeiten.
- **Übersicht:** jeder Arbeitsplatz mit Client-Version (veraltete markiert), Verbindungszustand
  und letzten Fehlern; Berichte ansehen; Issue mit Protokoll erstellen (GitHub, GitLab,
  Gitea/Forgejo).
- **Monitoring:** Prometheus-Metriken unter `/metrics`, standardmässig zusammengezählt.
- **Einfach zu betreiben:** ein Debian-Paket mit systemd-Dienst. Daten in SQLite, MariaDB oder
  PostgreSQL, nach Wahl des Admins. Kein Docker, keine fremden Dienste.

Kein Angebot der STARFACE GmbH. „STARFACE“ wird nur beschreibend verwendet.

> **Stand:** in Entwicklung. Ein Release gibt es noch nicht.

## Mitmachen

Fehlermeldungen, Ideen und Pull Requests sind willkommen, auf Deutsch oder Englisch. Siehe
[CONTRIBUTING.md](.github/CONTRIBUTING.md); Sicherheitslücken bitte vertraulich melden, wie in
[SECURITY.md](.github/SECURITY.md) beschrieben.

## Lizenz

[MIT](LICENSE)
