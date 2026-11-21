//! StarDash: Sammelstelle für Fehlerberichte und Statusmeldungen der
//! STARFACE-Clients StarCLX, StarCMX und StarMAC. Läuft bei der Firma als
//! systemd-Dienst, Daten in SQLite, MariaDB oder PostgreSQL.

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => println!("stardash {}", env!("CARGO_PKG_VERSION")),
        _ => {
            eprintln!(
                "stardash {}: noch in Entwicklung",
                env!("CARGO_PKG_VERSION")
            );
            std::process::exit(2);
        }
    }
}
