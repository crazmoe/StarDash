# Changelog

## Unreleased

- Erste Version der Sammelstelle:
  - Empfang von Fehlerberichten und Statusmeldungen der Clients (Schnittstelle v1,
    Anmeldung mit Firmen-Schlüssel)
  - Daten in SQLite, MariaDB oder PostgreSQL, Protokolle als Dateien oder in der
    Datenbank, Löschen nach 30 Tagen
  - Weboberfläche: Übersicht (Zustand, Versionen, Fehler der letzten 7 Tage),
    Arbeitsplätze mit Version und Zustand, Berichte mit Suche nach Bericht-Nr.,
    erledigt/Notiz, Rollen „support“ und „lesen“
  - Issue erstellen mit Protokoll: Vorgabe github.com (Repository der App), eigene
    Ziele GitHub, GitLab, Gitea/Forgejo, oder abschalten
  - Metriken für Prometheus unter `/metrics`, standardmässig zusammengezählt
  - Debian-Paket (amd64, arm64) mit systemd-Dienst und Abfrage der Datenbank
- Repository eingerichtet: README, Beitragshinweise, Issue-Vorlagen, CI.
