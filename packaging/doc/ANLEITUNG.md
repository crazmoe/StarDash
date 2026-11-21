# StarDash einrichten

StarDash sammelt Fehlerberichte und Statusmeldungen von StarCLX (Linux),
StarCMX (Windows) und StarMAC (macOS). Es läuft auf einem Debian- oder
Ubuntu-Server der Firma.

## 1. Installieren

```sh
sudo apt install ./stardash_<version>_amd64.deb
```

Bei der Installation fragt das Paket nach der Datenbank. SQLite braucht nichts
weiter. Für MariaDB oder PostgreSQL vorher Datenbank und Benutzer anlegen:

```sh
# MariaDB
sudo mysql -e "CREATE DATABASE stardash; CREATE USER 'stardash'@'localhost' IDENTIFIED BY 'PASSWORT'; GRANT ALL ON stardash.* TO 'stardash'@'localhost';"
# PostgreSQL
sudo -u postgres psql -c "CREATE USER stardash PASSWORD 'PASSWORT';" -c "CREATE DATABASE stardash OWNER stardash;"
```

Einstellungen stehen in `/etc/stardash/config.toml`, danach
`sudo systemctl restart stardash`.

## 2. Firma und Zugang anlegen

```sh
sudo stardash firma-anlegen "Meine Firma"   # zeigt den Schlüssel für die Clients
sudo stardash benutzer admin                # zeigt ein Passwort für die Weboberfläche
sudo stardash benutzer hotline lesen        # nur lesen, keine Issues
sudo stardash pruefen
```

## 3. HTTPS davor

StarDash lauscht nur auf `127.0.0.1:8470`. Davor gehört ein Webserver mit
Zertifikat; Beispiele liegen hier: `nginx.conf`, `apache.conf`, `Caddyfile`.

## 4. Clients einschalten

In die Vorgabedatei der Clients:

- Linux (StarCLX): `/etc/xdg/starclxrc`
- Windows (StarCMX): `C:\ProgramData\StarCMX\starcmxrc`

```ini
[Support]
url=https://stardash.firma.ch
token=<Schlüssel aus Schritt 2>
```

Die Clients lesen die Datei beim Start. Weitere Werte: `auto=` (wann
automatisch gesendet wird), `heartbeat=` (Minuten zwischen Statusmeldungen),
`internal_numbers=unkenntlich`.

## 5. Monitoring

`https://stardash.firma.ch/metrics` liefert Kennzahlen für Prometheus, Grafana,
Zabbix, Checkmk oder Icinga: Arbeitsplätze je Version und Zustand, angemeldete
Softphones, veraltete Versionen, Fehler heute. Mit `metrics_token` in der
config.toml ist ein Bearer-Token nötig.

## Datenschutz

- Die Clients machen Nummern, Namen, Chat-Texte und Zugangsdaten unkenntlich,
  bevor etwas den Rechner verlässt.
- StarDash speichert keine Benutzer, keine Präsenz, keine Gespräche und keine
  Gesprächszeiten. Daraus lässt sich nicht ablesen, wann oder wie viel jemand
  arbeitet; nutzen Sie die Daten auch nicht dafür (Schweiz: ArGV 3 Art. 26).
- Mitarbeitende informieren, z. B. im IT-Reglement: „Die Telefon-App sendet
  technische Fehlerprotokolle an den IT-Support (Server der Firma). Telefonnummern
  und Namen sind darin unkenntlich gemacht; Gesprächsinhalte werden nie gesendet.“
- Berichte werden nach `retention_days` (Standard 30 Tage) gelöscht.
- „Issue erstellen“ geht ab Werk an die öffentlichen Repositories der Clients
  auf github.com; vor dem Absenden zeigt StarDash eine Vorschau. Abschalten mit
  `[issues] modus = "aus"` oder eigene Ziele mit `modus = "eigene"`.
