#!/bin/sh
# Erzeugt THIRD-PARTY-LICENSES.txt: die Lizenzen aller Fremdkomponenten, die in
# StarDash stecken. Die Weboberfläche zeigt die Datei unter /lizenzen, das
# Debian-Paket legt sie unter /usr/share/doc/stardash/ ab. Die CI erzeugt sie
# vor jedem Paketbau neu und bricht ab, wenn eine Abhängigkeit eine Lizenz hat,
# die nicht in about.toml steht (z. B. GPL).
#
# Braucht: cargo install --locked cargo-about --features cli
#
#   sh scripts/third-party-licenses.sh
set -eu
cd "$(dirname "$0")/.."
out=THIRD-PARTY-LICENSES.txt
line="================================================================================"

rust=$(cargo about generate --fail -c about.toml about.hbs)

{
    printf '%s\n\n' "StarDash – Lizenzen der verwendeten Komponenten / Third-party licences"
    cat <<'TEXT'
StarDash selbst steht unter der MIT-Lizenz (siehe LICENSE). Es enthält die folgenden
Komponenten anderer Urheber unter deren Lizenzen.
StarDash itself is MIT licensed. It contains the following third-party components
under their own licences.

StarDash ist kein Produkt der STARFACE GmbH. „STARFACE“ wird nur beschreibend verwendet.
StarDash is not a product of STARFACE GmbH. "STARFACE" is used for description only.

TEXT
    printf '%s\n%s\n%s\n' "$line" "C-BIBLIOTHEKEN / NATIVE LIBRARIES" "$line"
    cat <<'TEXT'
SQLite – Public Domain
https://sqlite.org/copyright.html
--------------------------------------------------------------------------------
SQLite ist über das Crate libsqlite3-sys fest eingebunden. Der Quelltext von SQLite
ist gemeinfrei (public domain); es gelten keine Lizenzbedingungen.
SQLite is compiled in through the libsqlite3-sys crate. Its source code is in the
public domain.

TEXT
    printf '%s\n%s\n' "$line" "RUST-BIBLIOTHEKEN / RUST CRATES"
    printf '%s\n' "$rust"
} | sed 's/\r$//' > "$out"

echo "$out: $(($(wc -c < "$out") / 1024)) KB"
