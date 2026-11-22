//! StarDash: Sammelstelle für Fehlerberichte und Statusmeldungen der
//! STARFACE-Clients StarCLX, StarCMX und StarMAC. Läuft bei der Firma als
//! systemd-Dienst, Daten in SQLite, MariaDB oder PostgreSQL.

// Handler geben `Result<_, Response>` zurück (in axum üblich)
#![allow(clippy::result_large_err)]

mod api;
mod auth;
mod config;
mod db;
mod i18n;
mod issues;
mod metrics;
mod model;
mod releases;
mod web;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

use crate::config::{Config, ReportStore};
use crate::db::Db;
use crate::i18n::{Lang, fill, pick};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct App {
    pub config: Config,
    pub db: Db,
    /// Neueste veröffentlichte Version je App ("StarCLX" → "2.0.1")
    pub latest: RwLock<BTreeMap<String, String>>,
    pub http: reqwest::Client,
}

impl App {
    /// Datei mit dem gepackten Protokoll eines Berichts
    pub fn report_path(&self, company: i64, id: &str) -> PathBuf {
        self.config
            .data_dir
            .join("reports")
            .join(company.to_string())
            .join(format!("{id}.log.gz"))
    }

    /// Gepacktes Protokoll eines Berichts
    pub async fn report_log(&self, company: i64, id: &str) -> Option<Vec<u8>> {
        match self.config.database.reports {
            ReportStore::Datenbank => self.db.report_log(id).await.ok().flatten(),
            ReportStore::Dateien => tokio::fs::read(self.report_path(company, id)).await.ok(),
        }
    }

    pub fn latest(&self, app: &str) -> Option<String> {
        self.latest.read().ok()?.get(app).cloned()
    }
}

pub fn router(app: Arc<App>) -> Router {
    let limit = app.config.max_upload_mb as usize * 1024 * 1024;
    Router::new()
        .route("/api/v1/reports", post(api::report))
        .route("/api/v1/heartbeat", post(api::heartbeat))
        .route("/api/v1/health", get(api::health))
        .route("/metrics", get(metrics::metrics))
        .merge(web::routes())
        .layer(DefaultBodyLimit::max(limit))
        .layer(axum::middleware::from_fn_with_state(app.clone(), language))
        .with_state(app)
}

/// Sprache für die ganze Anfrage
async fn language(
    axum::extract::State(app): axum::extract::State<Arc<App>>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let lang = web::request_lang(&app.config.language, req.headers());
    i18n::scope(lang, next.run(req)).await
}

fn usage() -> ! {
    let text = pick(
        "StarDash {version}

Aufruf: stardash [--config DATEI] BEFEHL

Befehle:
  serve                         Dienst starten (macht systemd)
  firma-anlegen NAME            Firma anlegen, zeigt den Schlüssel für die Vorgabedatei
  firma-schluessel NAME         Neuen Schlüssel für eine Firma erzeugen
  firmen                        Firmen auflisten
  benutzer NAME [ROLLE] [FIRMA] Benutzer für die Weboberfläche anlegen oder
                                Passwort neu setzen (ROLLE: support oder lesen,
                                Standard support; FIRMA: Name, Standard alle)
  benutzer-liste                Benutzer der Weboberfläche auflisten
  benutzer-loeschen NAME        Benutzer löschen und abmelden
  pruefen                       Einstellungen, Datenbank und Datenordner prüfen
  --version                     Version zeigen

Alle Befehle gibt es auch auf Englisch (stardash help).
Einstellungen: {config}",
        "StarDash {version}

Usage: stardash [--config FILE] COMMAND

Commands:
  serve                         Run the service (done by systemd)
  create-company NAME           Create a company, shows the key for the policy file
  company-key NAME              Generate a new key for a company
  companies                     List companies
  user NAME [ROLE] [COMPANY]    Create a web user or reset the password
                                (ROLE: support or read, default support;
                                COMPANY: name, default all companies)
  users                         List web users
  delete-user NAME              Delete a user and sign them out
  check                         Check settings, database and data folder
  --version                     Show the version

All commands also exist in German (stardash hilfe).
Settings: {config}",
    );
    eprintln!(
        "{}",
        text.replace("{version}", VERSION)
            .replace("{config}", config::DEFAULT_PATH)
    );
    std::process::exit(2);
}

/// Englische Befehle auf die deutschen abbilden
fn command_name(c: &str) -> &str {
    match c {
        "create-company" => "firma-anlegen",
        "company-key" => "firma-schluessel",
        "companies" => "firmen",
        "user" => "benutzer",
        "users" => "benutzer-liste",
        "delete-user" => "benutzer-loeschen",
        "check" => "pruefen",
        other => other,
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .with_target(false)
        .init();
    if let Err(e) = run().await {
        eprintln!("{}: {e:#}", pick("Fehler", "Error"));
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    i18n::set_default(Lang::from_env());
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut config_path = PathBuf::from(config::DEFAULT_PATH);
    if let Some(i) = args.iter().position(|a| a == "--config" || a == "-c") {
        if i + 1 >= args.len() {
            usage();
        }
        config_path = PathBuf::from(args.remove(i + 1));
        args.remove(i);
    }
    let Some(command) = args.first().cloned() else {
        usage();
    };
    if matches!(command.as_str(), "help" | "hilfe") {
        i18n::set_default(if command == "help" {
            Lang::En
        } else {
            Lang::De
        });
    }
    if matches!(command.as_str(), "--version" | "-V" | "version") {
        println!("stardash {VERSION}");
        return Ok(());
    }
    if matches!(command.as_str(), "--help" | "-h" | "hilfe" | "help") {
        usage();
    }
    let config = Config::load(&config_path)?;
    if let Some(lang) = Lang::parse(&config.language) {
        i18n::set_default(lang);
    }
    let command = command_name(&command).to_string();
    let db = Db::connect(&config).await?;
    let result = command_run(&command, &args, &config_path, config.clone(), db).await;
    if command != "serve" {
        fix_owner(&config);
    }
    result
}

/// Mit `sudo stardash …` angelegte SQLite-Dateien gehören sonst root, und der
/// Dienst (Benutzer stardash) kann nicht mehr schreiben. Übernimmt den
/// Besitzer des Datenordners.
fn fix_owner(config: &Config) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let Ok(meta) = std::fs::metadata(&config.data_dir) else {
            return;
        };
        for name in ["stardash.db", "stardash.db-wal", "stardash.db-shm"] {
            let path = config.data_dir.join(name);
            if path.exists() {
                let _ = std::os::unix::fs::chown(&path, Some(meta.uid()), Some(meta.gid()));
            }
        }
    }
    #[cfg(not(unix))]
    let _ = config;
}

async fn command_run(
    command: &str,
    args: &[String],
    config_path: &std::path::Path,
    config: Config,
    db: Db,
) -> anyhow::Result<()> {
    match command {
        "serve" => serve(config, db).await,
        "firma-anlegen" => {
            let name = args.get(1).unwrap_or_else(|| usage());
            let token = auth::random_token(24);
            db.add_company(name, &auth::hash_token(&token)).await?;
            print_token(name, &token);
            Ok(())
        }
        "firma-schluessel" => {
            let name = args.get(1).unwrap_or_else(|| usage());
            let token = auth::random_token(24);
            if !db
                .set_company_token(name, &auth::hash_token(&token))
                .await?
            {
                anyhow::bail!(fill(
                    pick("Firma „{}“ nicht gefunden", "Company “{}” not found"),
                    &[name]
                ));
            }
            print_token(name, &token);
            println!(
                "{}",
                pick(
                    "Der alte Schlüssel gilt nicht mehr.",
                    "The old key no longer works."
                )
            );
            Ok(())
        }
        "firmen" => {
            for (id, name) in db.companies().await? {
                println!("{id}\t{name}");
            }
            Ok(())
        }
        "benutzer" => {
            let name = args.get(1).unwrap_or_else(|| usage());
            let role = match args.get(2).map(String::as_str).unwrap_or("support") {
                "support" => "support",
                "lesen" | "read" | "read-only" | "readonly" => "lesen",
                _ => anyhow::bail!(pick(
                    "Rolle muss support oder lesen sein",
                    "Role must be support or read"
                )),
            };
            let company = match args.get(3) {
                None => 0,
                Some(c) => db
                    .companies()
                    .await?
                    .into_iter()
                    .find(|(_, n)| n == c)
                    .map(|(id, _)| id)
                    .ok_or_else(|| {
                        anyhow::anyhow!(fill(
                            pick("Firma „{}“ nicht gefunden", "Company “{}” not found"),
                            &[c]
                        ))
                    })?,
            };
            let password = auth::random_token(9);
            db.add_user(name, &auth::hash_password(&password)?, role, company)
                .await?;
            let shown = if role == "lesen" {
                pick("lesen", "read")
            } else {
                role
            };
            println!(
                "{}",
                fill(
                    pick(
                        "Benutzer: {}\nPasswort: {}\nRolle:    {}",
                        "User:     {}\nPassword: {}\nRole:     {}"
                    ),
                    &[name, &password, &shown]
                )
            );
            Ok(())
        }
        "benutzer-liste" => {
            for (name, role, company) in db.users().await? {
                let company = if company.is_empty() {
                    pick("alle Firmen", "all companies")
                } else {
                    &company
                };
                let role = if role == "lesen" {
                    pick("lesen", "read")
                } else {
                    &role
                };
                println!("{name}\t{role}\t{company}");
            }
            Ok(())
        }
        "benutzer-loeschen" => {
            let name = args.get(1).unwrap_or_else(|| usage());
            if !db.remove_user(name).await? {
                anyhow::bail!(fill(
                    pick("Benutzer „{}“ nicht gefunden", "User “{}” not found"),
                    &[name]
                ));
            }
            println!(
                "{}",
                fill(pick("Benutzer {} gelöscht.", "User {} deleted."), &[name])
            );
            Ok(())
        }
        "pruefen" => {
            let label = |de: &'static str, en: &'static str| format!("{:<15}", pick(de, en));
            println!(
                "{}{} ok",
                label("Einstellungen:", "Settings:"),
                config_path.display()
            );
            println!(
                "{}{:?} ok",
                label("Datenbank:", "Database:"),
                config.database.kind
            );
            let test = config.data_dir.join(".schreibtest");
            tokio::fs::create_dir_all(&config.data_dir).await?;
            tokio::fs::write(&test, b"ok").await?;
            tokio::fs::remove_file(&test).await?;
            println!(
                "{}{} {}",
                label("Datenordner:", "Data folder:"),
                config.data_dir.display(),
                pick("beschreibbar", "writable")
            );
            println!(
                "{}{}",
                label("Firmen:", "Companies:"),
                db.companies().await?.len()
            );
            println!("{}{}", label("Benutzer:", "Users:"), db.user_count().await?);
            Ok(())
        }
        _ => usage(),
    }
}

fn print_token(name: &str, token: &str) {
    println!(
        "{}",
        fill(
            pick(
                "Firma: {}\nSchlüssel: {}\n\nIn die Vorgabedatei der Clients (starclxrc / starcmxrc):\n\n[Support]\nurl=https://<Adresse von StarDash>\ntoken={}\n\nDer Schlüssel wird nur gehasht gespeichert und lässt sich nicht noch einmal anzeigen.",
                "Company: {}\nKey: {}\n\nAdd to the clients' policy file (starclxrc / starcmxrc):\n\n[Support]\nurl=https://<address of StarDash>\ntoken={}\n\nThe key is stored only as a hash and cannot be shown again.",
            ),
            &[&name, &token, &token]
        )
    );
}

async fn serve(config: Config, db: Db) -> anyhow::Result<()> {
    if db.user_count().await? == 0 {
        tracing::warn!("Noch kein Benutzer für die Weboberfläche: stardash benutzer NAME");
    }
    let listen = config.listen;
    let app = Arc::new(App {
        http: reqwest::Client::builder()
            .user_agent(format!("stardash/{VERSION}"))
            .timeout(std::time::Duration::from_secs(30))
            .build()?,
        config,
        db,
        latest: RwLock::default(),
    });
    // Aufräumen und Versionen nachsehen
    let bg = app.clone();
    tokio::spawn(async move {
        loop {
            match bg.db.cleanup(bg.config.retention_days).await {
                Ok(old) => {
                    for (company, id) in &old {
                        let _ = tokio::fs::remove_file(bg.report_path(*company, id)).await;
                    }
                    if !old.is_empty() {
                        tracing::info!(n = old.len(), "Alte Berichte gelöscht");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "Aufräumen fehlgeschlagen"),
            }
            releases::refresh(&bg).await;
            tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
        }
    });
    let listener = tokio::net::TcpListener::bind(listen).await?;
    tracing::info!(%listen, version = VERSION, "StarDash gestartet");
    axum::serve(listener, router(app))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

/// Beenden bei Ctrl+C oder SIGTERM (systemctl stop)
async fn shutdown() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("SIGTERM abfangen");
        tokio::select! {
            _ = ctrl_c => {},
            _ = term.recv() => {},
        }
    }
    #[cfg(not(unix))]
    let _ = ctrl_c.await;
    tracing::info!("StarDash beendet");
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::io::Write;
    use tower::ServiceExt;

    /// Ganzer Ablauf gegen eine Datenbank: Firma, Bericht, Statusmeldung,
    /// Anmeldung, Seiten, Metriken.
    async fn roundtrip(kind: &str, url: &str) {
        let dir = std::env::temp_dir().join(format!("stardash-test-{kind}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut config: Config = toml::from_str(&format!(
            "data_dir = \"{}\"\ncheck_releases = false\n[datenbank]\nart = \"{kind}\"\nadresse = \"{url}\"\nberichte = \"{}\"",
            dir.display(),
            if kind == "postgresql" { "datenbank" } else { "dateien" }
        ))
        .unwrap();
        config.metrics_per_host = true;
        let db = Db::connect(&config).await.unwrap();
        let run = auth::random_token(3);
        let company = format!("Test {run}");
        let token = auth::random_token(24);
        db.add_company(&company, &auth::hash_token(&token))
            .await
            .unwrap();
        db.add_user("admin", &auth::hash_password("pw").unwrap(), "support", 0)
            .await
            .unwrap();
        // Benutzer mit Firma, neues Passwort beendet Anmeldung, löschen
        let reader = format!("leser-{run}");
        let pw = auth::hash_password("pw").unwrap();
        let cid = db
            .companies()
            .await
            .unwrap()
            .into_iter()
            .find(|(_, n)| *n == company)
            .unwrap()
            .0;
        db.add_user(&reader, &pw, "lesen", cid).await.unwrap();
        let (uid, _) = db.user_login(&reader).await.unwrap().unwrap();
        db.add_session(&format!("s-{run}"), uid, db::now() + 60)
            .await
            .unwrap();
        db.add_user(&reader, &pw, "lesen", cid).await.unwrap();
        assert!(
            db.session_user(&format!("s-{run}"))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            db.users()
                .await
                .unwrap()
                .iter()
                .any(|(n, r, c)| *n == reader && r == "lesen" && *c == company)
        );
        assert!(
            db.users()
                .await
                .unwrap()
                .iter()
                .any(|(n, _, c)| n == "admin" && c.is_empty())
        );
        assert!(db.remove_user(&reader).await.unwrap());
        assert!(!db.remove_user(&reader).await.unwrap());
        let app = Arc::new(App {
            config,
            db,
            latest: RwLock::new(BTreeMap::from([(
                "StarCLX".to_string(),
                "2.0.1".to_string(),
            )])),
            http: reqwest::Client::new(),
        });
        let router = router(app.clone());
        let id = format!(
            "K7-{}",
            &auth::random_token(2)
                .to_uppercase()
                .replace(['0', '1'], "Z")[..4]
        );
        let id = id
            .chars()
            .map(|c| if "OI".contains(c) { 'Z' } else { c })
            .collect::<String>();
        let report = serde_json::json!({
            "id": id, "reason": "sip", "time": 1_795_300_000u64,
            "client": {"install_id": format!("inst-{kind}-{run}"), "hostname": "PC-EMPFANG", "app": "StarCLX", "version": "2.0.0", "os": "linux x86_64; Debian 13", "package": "deb", "pbx": "https://pbx", "pbx_version": "10.0.2.7"},
            "comment": "Anruf brach ab", "summary": "Softphone seit 2 Minuten abgemeldet",
            "log": "2026-11-21T22:00:00Z ERROR sf_sip: Registrierung 403\nzeile 2\n"
        });
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(report.to_string().as_bytes()).unwrap();
        let packed = gz.finish().unwrap();
        let send = |path: &str, body: Vec<u8>, gzip: bool, tok: &str| {
            let mut b = Request::post(path)
                .header("authorization", format!("Bearer {tok}"))
                .header("content-type", "application/json");
            if gzip {
                b = b.header("content-encoding", "gzip");
            }
            b.body(Body::from(body)).unwrap()
        };
        // falscher Schlüssel
        let res = router
            .clone()
            .oneshot(send("/api/v1/reports", packed.clone(), true, "nope"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        let res = router
            .clone()
            .oneshot(send("/api/v1/reports", packed.clone(), true, &token))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{kind}");
        // nochmals (Warteordner des Clients) ist kein Fehler
        let res = router
            .clone()
            .oneshot(send("/api/v1/reports", packed, true, &token))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let hb = serde_json::json!({
            "client": report["client"], "time": 1, "event": "start", "pbx_connected": true,
            "chat_connected": true, "softphone_registered": false, "softphone_enabled": true,
            "audio_ok": true, "uptime_secs": 60, "last_error": 0, "errors": {"sip": 2}
        });
        let res = router
            .clone()
            .oneshot(send(
                "/api/v1/heartbeat",
                hb.to_string().into_bytes(),
                false,
                &token,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT, "{kind}");
        let res = router
            .clone()
            .oneshot(send(
                "/api/v1/heartbeat",
                hb.to_string().into_bytes(),
                false,
                &token,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);

        let r = app.db.report(&id).await.unwrap().unwrap();
        assert_eq!(r.hostname, "PC-EMPFANG");
        let log = app.report_log(r.company, &id).await.unwrap();
        assert!(!log.is_empty());
        let clients = app.db.clients(None).await.unwrap();
        let c = clients
            .iter()
            .find(|c| c.install_id == format!("inst-{kind}-{run}"))
            .unwrap();
        assert!(c.heartbeat && !c.softphone_registered && c.reports_24h == 1);
        // Erledigte Berichte zählen nicht mehr als Fehler
        app.db.set_report_state(&id, true, "").await.unwrap();
        let clients = app.db.clients(None).await.unwrap();
        let c = clients
            .iter()
            .find(|c| c.install_id == format!("inst-{kind}-{run}"))
            .unwrap();
        assert_eq!(c.reports_24h, 0);
        app.db.set_report_state(&id, false, "").await.unwrap();
        let errors = app.db.errors(None, 1).await.unwrap();
        assert!(
            errors.iter().any(|(_, _, _, k, n)| k == "sip" && *n >= 4),
            "{errors:?}"
        );

        // Weboberfläche: ohne Anmeldung zur Anmeldung
        let res = router
            .clone()
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let res = router
            .clone()
            .oneshot(
                Request::post("/login")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("name=admin&password=pw"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let cookie = res.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        for path in [
            "/",
            "/arbeitsplaetze",
            "/berichte",
            &format!("/berichte/{id}"),
            &format!("/berichte/{id}/issue"),
            &format!("/berichte/{id}/protokoll"),
        ] {
            let res = router
                .clone()
                .oneshot(
                    Request::get(path)
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "{path}");
            let body =
                String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
                    .unwrap();
            if path == "/" {
                assert!(body.contains("veraltet"), "Version 2.0.0 < 2.0.1 markiert");
            }
            if path.ends_with("/protokoll") {
                assert!(body.contains("Registrierung 403"));
            }
        }
        let res = router
            .clone()
            .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        assert!(
            body.contains("stardash_clients{app=\"StarCLX\",version=\"2.0.0\",zustand=\"fehler\"}"),
            "{body}"
        );
        assert!(body.contains("stardash_clients_veraltet{app=\"StarCLX\"} "));
        assert!(body.contains("stardash_client_ok{rechner=\"PC-EMPFANG\""));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn sqlite() {
        roundtrip("sqlite", "").await;
    }

    /// Nur mit laufender Datenbank: STARDASH_TEST_MARIADB=mysql://…
    #[tokio::test]
    async fn mariadb() {
        if let Ok(url) = std::env::var("STARDASH_TEST_MARIADB") {
            roundtrip("mariadb", &url).await;
        }
    }

    /// Nur mit laufender Datenbank: STARDASH_TEST_POSTGRES=postgres://…
    #[tokio::test]
    async fn postgresql() {
        if let Ok(url) = std::env::var("STARDASH_TEST_POSTGRES") {
            roundtrip("postgresql", &url).await;
        }
    }
}
