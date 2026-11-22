//! Sprache der Weboberfläche und der Befehle: Deutsch oder Englisch.
//!
//! Texte stehen im Code auf Deutsch; [`t`] liefert die englische Fassung aus
//! [`EN`]. Die Sprache gilt pro Anfrage (Cookie, `language` in der
//! config.toml, Sprache des Browsers) bzw. für die Befehlszeile (config.toml,
//! sonst `LANG`).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    De,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().to_ascii_lowercase();
        if s.starts_with("de") {
            Some(Lang::De)
        } else if s.starts_with("en") {
            Some(Lang::En)
        } else {
            None
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Lang::De => "de",
            Lang::En => "en",
        }
    }

    /// Erste passende Sprache aus `Accept-Language`
    pub fn from_accept(header: &str) -> Option<Self> {
        let mut tags: Vec<(f32, &str)> = header
            .split(',')
            .map(|part| {
                let mut it = part.split(';');
                let tag = it.next().unwrap_or("").trim();
                let q = it
                    .find_map(|p| p.trim().strip_prefix("q="))
                    .and_then(|q| q.parse().ok())
                    .unwrap_or(1.0);
                (q, tag)
            })
            .collect();
        tags.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        tags.into_iter().find_map(|(_, tag)| Lang::parse(tag))
    }

    /// Sprache der Befehlszeile aus der Umgebung
    pub fn from_env() -> Self {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .find(|v| !v.is_empty())
            .and_then(|v| Lang::parse(&v))
            .unwrap_or(Lang::En)
    }
}

tokio::task_local! {
    static CURRENT: Lang;
}

static DEFAULT: AtomicU8 = AtomicU8::new(0);

/// Sprache ausserhalb einer Anfrage (Befehlszeile)
pub fn set_default(lang: Lang) {
    DEFAULT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    CURRENT.try_with(|l| *l).unwrap_or_else(|_| {
        if DEFAULT.load(Ordering::Relaxed) == Lang::En as u8 {
            Lang::En
        } else {
            Lang::De
        }
    })
}

/// Führt `f` in der Sprache `lang` aus
pub async fn scope<F: std::future::Future>(lang: Lang, f: F) -> F::Output {
    CURRENT.scope(lang, f).await
}

/// Text in der aktuellen Sprache
pub fn t(de: &'static str) -> &'static str {
    match current() {
        Lang::De => de,
        Lang::En => EN
            .iter()
            .find(|(k, _)| *k == de)
            .map(|(_, v)| *v)
            .unwrap_or(de),
    }
}

/// Kurzer Text direkt in beiden Sprachen (Befehlszeile)
pub fn pick<'a>(de: &'a str, en: &'a str) -> &'a str {
    match current() {
        Lang::De => de,
        Lang::En => en,
    }
}

/// Setzt Werte nacheinander für `{}` ein
pub fn fill(template: &str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut args = args.iter();
    let mut rest = template;
    while let Some(i) = rest.find("{}") {
        out.push_str(&rest[..i]);
        if let Some(a) = args.next() {
            out.push_str(&a.to_string());
        }
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// Deutsch → Englisch
pub const EN: &[(&str, &str)] = &[
    // Allgemein
    ("gerade eben", "just now"),
    ("vor {} min", "{} min ago"),
    ("vor {} h", "{} h ago"),
    ("vor {} Tagen", "{} days ago"),
    ("Übersicht", "Overview"),
    ("Arbeitsplätze", "Workstations"),
    ("Berichte", "Reports"),
    ("Abmelden", "Sign out"),
    (
        "zeigt den Zustand der Programme, nicht das Verhalten von Personen",
        "shows the state of the apps, not the behaviour of people",
    ),
    ("Nicht gefunden", "Not found"),
    ("Fehler", "Error"),
    ("support", "support"),
    ("lesen", "read-only"),
    ("ja", "yes"),
    ("nein", "no"),
    ("zurücksetzen", "reset"),
    ("Zurück", "Back"),
    ("Speichern", "Save"),
    ("Abbrechen", "Cancel"),
    // Anmeldung
    ("Anmelden", "Sign in"),
    ("Benutzer", "User"),
    ("Passwort", "Password"),
    ("Benutzer oder Passwort falsch.", "Wrong user or password."),
    (
        "Noch kein Benutzer. Auf dem Server anlegen mit <code>sudo stardash benutzer NAME</code>.",
        "No user yet. Create one on the server with <code>sudo stardash user NAME</code>.",
    ),
    // Zustand
    ("In Ordnung", "OK"),
    ("Nicht erreichbar", "Unreachable"),
    ("Nur Berichte", "Reports only"),
    ("Anlage", "PBX"),
    ("Anmeldung", "Sign-in"),
    ("Verbindung", "Connection"),
    // Anlässe
    ("Vom Benutzer gesendet", "Sent by user"),
    ("Absturz", "Crash"),
    ("Softphone", "Softphone"),
    ("Nach Standby", "After standby"),
    // Übersicht
    ("Neueste Version: {}", "Latest version: {}"),
    ("veraltet", "outdated"),
    ("Versionen im Einsatz", "Versions in use"),
    (
        "Noch keine Meldungen. Adresse und Schlüssel in die Vorgabedatei der Clients eintragen (<code>[Support]</code>).",
        "No reports yet. Add the address and key to the clients' policy file (<code>[Support]</code>).",
    ),
    ("Neueste Versionen: {}", "Latest versions: {}"),
    ("Fehler der letzten 7 Tage", "Errors in the last 7 days"),
    ("{} Fehler", "{} errors"),
    ("Nach Art: {}", "By kind: {}"),
    ("Nach Version: {}", "By version: {}"),
    ("Neueste Berichte", "Latest reports"),
    // Tabellen
    ("Nr.", "No."),
    ("Erhalten", "Received"),
    ("Anlass", "Reason"),
    ("Arbeitsplatz", "Workstation"),
    ("Text", "Text"),
    ("erledigt", "done"),
    ("Keine Berichte.", "No reports."),
    ("Rechnername", "Computer name"),
    ("Alle Zustände", "All states"),
    ("Filtern", "Filter"),
    ("Rechner", "Computer"),
    ("Zustand", "State"),
    ("Letzte Meldung", "Last seen"),
    ("Offene Berichte 24 h", "Open reports 24 h"),
    ("Keine Arbeitsplätze.", "No workstations."),
    ("{} von {} Arbeitsplätzen", "{} of {} workstations"),
    ("Paket", "Package"),
    ("Softphone eingerichtet", "Softphone set up"),
    ("Läuft seit", "Running for"),
    ("Letzter Fehler", "Last error"),
    ("Zuerst gesehen", "First seen"),
    ("Installations-Kennung", "Installation ID"),
    // Berichte
    (
        "Bericht-Nr., Rechner oder Version",
        "Report no., computer or version",
    ),
    ("Alle Anlässe", "All reasons"),
    ("nur offene", "open only"),
    ("Suchen", "Search"),
    ("Bericht", "Report"),
    ("Zeit", "Time"),
    ("{} (erhalten {})", "{} (received {})"),
    ("Meldung", "Message"),
    ("Text des Benutzers", "User's text"),
    ("Protokoll", "Log"),
    ("herunterladen", "download"),
    ("Bearbeitung", "Handling"),
    ("Notiz für den Support", "Note for support"),
    ("Notiz", "Note"),
    ("Issue erstellen", "Create issue"),
    ("Fehler und Warnungen", "Errors and warnings"),
    (
        "Gezeigt wird der neueste Teil; das ganze Protokoll gibt es zum Herunterladen.",
        "Showing the newest part; download the full log.",
    ),
    // Issue
    ("Ziel", "Target"),
    ("Wählen", "Choose"),
    (
        "<strong>Öffentlich:</strong> Issues in diesem Repository kann jeder im Internet lesen. Prüfe Titel, Text und Protokoll unten, bevor du absendest. Interne Nummern stehen standardmässig im Klartext; mit dem Häkchen werden sie unkenntlich.",
        "<strong>Public:</strong> anyone on the internet can read issues in this repository. Check title, text and log below before you submit. Internal numbers are readable by default; tick the box to hide them.",
    ),
    (
        "Für dieses Ziel ist kein Token eingetragen: StarDash öffnet das vorausgefüllte Issue in deinem Browser, wo du es mit deinem GitHub-Konto absendest. Das Protokoll lädst du vorher herunter und hängst es dort an.",
        "No token is set for this target: StarDash opens the prefilled issue in your browser, where you submit it with your GitHub account. Download the log first and attach it there.",
    ),
    ("Titel", "Title"),
    (
        "Protokoll (wird angehängt; du kannst Zeilen entfernen)",
        "Log (will be attached; you can remove lines)",
    ),
    (
        "Interne Nummern unkenntlich machen",
        "Hide internal numbers",
    ),
    (
        "Issue im Browser absenden",
        "Submit the issue in the browser",
    ),
    ("Issue im Browser", "Issue in browser"),
    ("Protokoll herunterladen", "Download log"),
    ("Vorausgefülltes Issue öffnen", "Open prefilled issue"),
    (
        "Dort die Protokolldatei in das Textfeld ziehen und absenden.",
        "Drag the log file into the text field there and submit.",
    ),
    (
        "Den Link des Issues hier eintragen:",
        "Enter the issue's link here:",
    ),
    (
        "(Optional) Link zum Issue als Notiz speichern:",
        "(Optional) Save the link to the issue as a note:",
    ),
    ("Issue nicht erstellt", "Issue not created"),
    (
        "Entwickler von StarCLX/StarCMX/StarMAC",
        "Developers of StarCLX/StarCMX/StarMAC",
    ),
    (
        "**Was ist passiert (Benutzer):**",
        "**What happened (user):**",
    ),
    ("Betriebssystem", "Operating system"),
    ("**Meldung:**", "**Message:**"),
    ("**Fehler und Warnungen:**", "**Errors and warnings:**"),
    ("**Protokoll:**", "**Log:**"),
    ("(leer)", "(empty)"),
    // Lizenzen
    ("Lizenzen", "Licences"),
    ("Fremdkomponenten", "Third-party components"),
    (
        "StarDash ist kein Produkt der STARFACE GmbH. „STARFACE“ wird nur beschreibend verwendet.",
        "StarDash is not a product of STARFACE GmbH. “STARFACE” is used for description only.",
    ),
    (
        "StarDash enthält die folgenden Komponenten anderer Urheber unter deren Lizenzen.",
        "StarDash contains the following third-party components under their own licences.",
    ),
    (
        "bitte die Datei `{}` aus StarDash herunterladen und hier anhängen.",
        "please download the file `{}` from StarDash and attach it here.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_language() {
        assert_eq!(Lang::from_accept("de-CH,de;q=0.9,en;q=0.8"), Some(Lang::De));
        assert_eq!(
            Lang::from_accept("fr-CH, en;q=0.5, de;q=0.3"),
            Some(Lang::En)
        );
        assert_eq!(Lang::from_accept("fr"), None);
    }

    #[tokio::test]
    async fn translate() {
        assert_eq!(t("Berichte"), "Berichte");
        scope(Lang::En, async {
            assert_eq!(t("Berichte"), "Reports");
            assert_eq!(fill(t("vor {} min"), &[&5]), "5 min ago");
            assert_eq!(t("gibt es nicht"), "gibt es nicht");
        })
        .await;
    }

    /// Jeder Text in `t("…")` braucht eine englische Fassung
    #[test]
    fn all_translated() {
        let re = regex::Regex::new(r#"\b(?:t|tr)\(\s*"((?:[^"\\]|\\.)*)""#).unwrap();
        let mut missing = Vec::new();
        for src in [
            include_str!("web.rs"),
            include_str!("issues.rs"),
            include_str!("metrics.rs"),
        ] {
            for c in re.captures_iter(src) {
                let de = c[1].replace("\\\"", "\"");
                if !EN.iter().any(|(k, _)| *k == de) {
                    missing.push(de);
                }
            }
        }
        for (_, l) in crate::model::REASONS {
            if !EN.iter().any(|(k, _)| k == l) {
                missing.push(l.to_string());
            }
        }
        assert!(missing.is_empty(), "ohne Übersetzung: {missing:?}");
    }

    #[test]
    fn no_duplicates() {
        let mut keys: Vec<&str> = EN.iter().map(|(k, _)| *k).collect();
        keys.sort();
        let n = keys.len();
        keys.dedup();
        assert_eq!(n, keys.len());
    }
}
