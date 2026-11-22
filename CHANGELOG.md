# Changelog

## 1.0.0

Erste Version / first release.

- Empfang von Fehlerberichten und Statusmeldungen der Clients StarCLX und
  StarCMX ab 2.0.0 (Schnittstelle v1, Anmeldung mit Firmen-Schlüssel)
- Daten in SQLite, MariaDB oder PostgreSQL, Protokolle als Dateien oder in der
  Datenbank, Löschen nach 30 Tagen
- Weboberfläche: Übersicht (Zustand, Versionen, Fehler der letzten 7 Tage),
  Arbeitsplätze mit Version und Zustand, Berichte mit Suche nach Bericht-Nr.,
  erledigt/Notiz (erledigte Berichte zählen nicht mehr als Fehler), Rollen
  „support“ und „lesen“
- Deutsch und Englisch: Weboberfläche nach Browser oder Umschalter, Befehle und
  config.toml auch mit englischen Namen, Anleitung in beiden Sprachen
- Issue erstellen mit Protokoll: Vorgabe github.com (Repository der App), eigene
  Ziele GitHub, GitLab, Gitea/Forgejo, oder abschalten
- Metriken für Prometheus unter `/metrics`, standardmässig zusammengezählt
- Befehle: Firmen und Schlüssel, Benutzer anlegen, auflisten, löschen, Passwort
  zurücksetzen (meldet in allen Browsern ab), Prüfung der Einrichtung
- Debian-Paket (amd64, arm64) mit systemd-Dienst und Abfrage der Datenbank
- Lizenzen der Fremdkomponenten im Paket und unter „Lizenzen“ in der
  Weboberfläche; CI bricht bei nicht zugelassenen Lizenzen (z. B. GPL) ab

English summary: receives error reports and status messages from StarCLX and
StarCMX 2.0.0 and later; SQLite, MariaDB or PostgreSQL; web interface in
English and German; issues to GitHub, GitLab, Gitea/Forgejo; Prometheus
metrics; Debian package for amd64 and arm64; third-party licences included.
