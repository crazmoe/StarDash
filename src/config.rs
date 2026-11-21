//! Einstellungen aus `/etc/stardash/config.toml`.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const DEFAULT_PATH: &str = "/etc/stardash/config.toml";

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Adresse, auf der StarDash lauscht. Standard nur lokal: davor gehört ein
    /// Webserver mit HTTPS (nginx, Apache, Caddy).
    pub listen: SocketAddr,
    /// Datenordner (Berichte als Dateien, SQLite-Datei)
    pub data_dir: PathBuf,
    /// Berichte nach so vielen Tagen löschen
    pub retention_days: u32,
    /// Grösste Anfrage eines Clients in MB (gepackt)
    pub max_upload_mb: u32,
    /// Ohne Statusmeldung seit so vielen Minuten gilt ein Client als nicht
    /// erreichbar
    pub offline_after_minutes: u32,
    /// Einmal am Tag die neueste Client-Version auf GitHub nachsehen
    pub check_releases: bool,
    /// Metriken einzelner Rechner ausgeben (Standard: nur zusammengezählt)
    pub metrics_per_host: bool,
    /// Zugang zu `/metrics`: leer = ohne Anmeldung, sonst Bearer-Token
    pub metrics_token: String,
    #[serde(rename = "datenbank")]
    pub database: Database,
    pub issues: Issues,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: "127.0.0.1:8470".parse().expect("gültige Adresse"),
            data_dir: PathBuf::from("/var/lib/stardash"),
            retention_days: 30,
            max_upload_mb: 20,
            offline_after_minutes: 45,
            check_releases: true,
            metrics_per_host: false,
            metrics_token: String::new(),
            database: Database::default(),
            issues: Issues::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Sqlite,
    Mariadb,
    Mysql,
    #[serde(alias = "postgres")]
    Postgresql,
}

impl Kind {
    pub fn is_mysql(self) -> bool {
        matches!(self, Kind::Mariadb | Kind::Mysql)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ReportStore {
    /// Als gepackte Dateien im Datenordner (Standard)
    #[default]
    Dateien,
    /// In der Datenbank
    Datenbank,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Database {
    /// sqlite (Standard), mariadb oder postgresql
    #[serde(rename = "art")]
    pub kind: Kind,
    /// Verbindung für MariaDB/PostgreSQL, z. B.
    /// `mysql://stardash:geheim@localhost/stardash` oder
    /// `postgres://stardash:geheim@localhost/stardash`
    #[serde(rename = "adresse")]
    pub url: String,
    /// Wo die Protokolle der Berichte liegen
    #[serde(rename = "berichte")]
    pub reports: ReportStore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum IssueMode {
    /// github.com crazmoe/StarCLX bzw. crazmoe/StarCMX (bzw. StarMAC)
    #[default]
    Vorgabe,
    /// Kein Knopf „Issue erstellen“
    Aus,
    /// Nur die eigenen Ziele
    Eigene,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
pub struct Issues {
    #[serde(rename = "modus")]
    pub mode: IssueMode,
    /// Token für die Vorgabe (github.com), damit StarDash Issue und Gist
    /// selbst anlegt. Ohne Token öffnet StarDash ein vorausgefülltes Issue im
    /// Browser des Supports.
    pub github_token: String,
    #[serde(rename = "ziel")]
    pub targets: Vec<IssueTarget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueTarget {
    /// Anzeigename, z. B. "IT Telefonie"
    #[serde(rename = "name", default)]
    pub name: String,
    /// github, gitlab, gitea (auch Forgejo)
    #[serde(rename = "art")]
    pub kind: TargetKind,
    /// Adresse der Instanz (GitHub: leer = github.com)
    #[serde(default)]
    pub url: String,
    /// GitHub/Gitea: owner/repo; GitLab: Pfad oder ID des Projekts
    #[serde(rename = "projekt")]
    pub project: String,
    #[serde(default)]
    pub token: String,
    /// Nur für diese Apps vorschlagen (leer = alle)
    #[serde(default)]
    pub apps: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TargetKind {
    Github,
    Gitlab,
    #[serde(alias = "forgejo")]
    Gitea,
}

impl Config {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(path = %path.display(), "Keine Konfiguration, nehme Standardwerte");
                String::new()
            }
            Err(e) => return Err(anyhow::anyhow!("{}: {e}", path.display())),
        };
        let config: Config =
            toml::from_str(&text).map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        if config.database.kind != Kind::Sqlite && config.database.url.is_empty() {
            anyhow::bail!("[datenbank] adresse fehlt für {:?}", config.database.kind);
        }
        Ok(config)
    }

    /// Verbindungsadresse für sqlx
    pub fn database_url(&self) -> String {
        match self.database.kind {
            Kind::Sqlite => format!(
                "sqlite://{}?mode=rwc",
                self.data_dir.join("stardash.db").display()
            ),
            _ => self.database.url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_example() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.database.kind, Kind::Sqlite);
        assert!(c.database_url().starts_with("sqlite:///var/lib/stardash/"));
        let c: Config = toml::from_str(include_str!("../packaging/config.toml")).unwrap();
        assert_eq!(c.retention_days, 30);
        let c: Config = toml::from_str(
            r#"
            [datenbank]
            art = "postgresql"
            adresse = "postgres://x@localhost/stardash"
            berichte = "datenbank"
            [issues]
            modus = "eigene"
            [[issues.ziel]]
            art = "gitlab"
            url = "https://gitlab.firma.ch"
            projekt = "it/telefonie"
            token = "t"
            "#,
        )
        .unwrap();
        assert_eq!(c.database.kind, Kind::Postgresql);
        assert_eq!(c.database.reports, ReportStore::Datenbank);
        assert_eq!(c.issues.targets[0].kind, TargetKind::Gitlab);
    }
}
