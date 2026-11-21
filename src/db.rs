//! Datenbank: SQLite, MariaDB/MySQL oder PostgreSQL über `sqlx::Any`.
//!
//! Die Tabellen sind bewusst einfach (Zahlen und Text), damit dieselben
//! Abfragen überall laufen. Platzhalter schreiben wir als `?`; für PostgreSQL
//! werden sie in `$1, $2, …` umgeschrieben. Zeiten sind Unix-Sekunden,
//! Ja/Nein-Werte 0/1.

use std::collections::BTreeMap;

use anyhow::Context;
use serde::Serialize;
use sqlx::any::{AnyPoolOptions, AnyRow};
use sqlx::{AnyPool, Row};

use crate::config::{Config, Kind};
use crate::model::{Heartbeat, Report};

/// Version des Schemas; bei Änderungen erhöhen und in [`migrate`] ergänzen
const SCHEMA: i64 = 1;

#[derive(Clone)]
pub struct Db {
    pool: AnyPool,
    kind: Kind,
}

/// Spaltentypen je Datenbank
struct Types {
    id: &'static str,
    key: &'static str,
    text: &'static str,
    long_text: &'static str,
    blob: &'static str,
    int: &'static str,
}

impl Db {
    pub async fn connect(config: &Config) -> anyhow::Result<Self> {
        sqlx::any::install_default_drivers();
        if config.database.kind == Kind::Sqlite {
            std::fs::create_dir_all(&config.data_dir)
                .with_context(|| format!("{}", config.data_dir.display()))?;
        }
        let pool = AnyPoolOptions::new()
            .max_connections(if config.database.kind == Kind::Sqlite {
                1
            } else {
                8
            })
            .connect(&config.database_url())
            .await
            .context("Datenbank nicht erreichbar")?;
        let db = Db {
            pool,
            kind: config.database.kind,
        };
        if db.kind == Kind::Sqlite {
            sqlx::query("PRAGMA journal_mode=WAL")
                .execute(&db.pool)
                .await?;
        }
        db.migrate().await?;
        Ok(db)
    }

    fn types(&self) -> Types {
        match self.kind {
            Kind::Sqlite => Types {
                id: "INTEGER PRIMARY KEY AUTOINCREMENT",
                key: "TEXT",
                text: "TEXT",
                long_text: "TEXT",
                blob: "BLOB",
                int: "BIGINT",
            },
            Kind::Mariadb | Kind::Mysql => Types {
                id: "BIGINT PRIMARY KEY AUTO_INCREMENT",
                key: "VARCHAR(191)",
                text: "VARCHAR(1024)",
                long_text: "MEDIUMTEXT",
                blob: "LONGBLOB",
                int: "BIGINT",
            },
            Kind::Postgresql => Types {
                id: "BIGSERIAL PRIMARY KEY",
                key: "VARCHAR(191)",
                text: "TEXT",
                long_text: "TEXT",
                blob: "BYTEA",
                int: "BIGINT",
            },
        }
    }

    /// Lange Texte liest MariaDB über `sqlx::Any` sonst als BLOB
    fn text_col(&self, col: &str) -> String {
        if self.kind.is_mysql() {
            format!("CAST({col} AS CHAR)")
        } else {
            col.to_string()
        }
    }

    /// Summen sind in PostgreSQL NUMERIC, das `sqlx::Any` nicht kennt
    fn int_expr(&self, expr: &str) -> String {
        if self.kind.is_mysql() {
            format!("CAST({expr} AS SIGNED)")
        } else {
            format!("CAST({expr} AS BIGINT)")
        }
    }

    fn report_select(&self) -> String {
        format!(
            "SELECT id, company_id, install_id, reason, time, received, hostname, app, version, os, package, pbx, pbx_version, {}, {}, log_size, done, {}, issue_url FROM reports",
            self.text_col("comment"),
            self.text_col("summary"),
            self.text_col("note")
        )
    }

    /// `?` → `$n` für PostgreSQL
    fn sql(&self, q: &str) -> String {
        if self.kind != Kind::Postgresql {
            return q.to_string();
        }
        let mut out = String::with_capacity(q.len() + 8);
        let mut n = 0;
        for c in q.chars() {
            if c == '?' {
                n += 1;
                out.push('$');
                out.push_str(&n.to_string());
            } else {
                out.push(c);
            }
        }
        out
    }

    async fn migrate(&self) -> anyhow::Result<()> {
        let t = self.types();
        let int = t.int;
        sqlx::query(&format!(
            "CREATE TABLE IF NOT EXISTS schema_version (version {int} NOT NULL)"
        ))
        .execute(&self.pool)
        .await?;
        let current: i64 = sqlx::query("SELECT version FROM schema_version")
            .fetch_optional(&self.pool)
            .await?
            .map(|r| r.get::<i64, _>(0))
            .unwrap_or(0);
        if current >= SCHEMA {
            return Ok(());
        }
        let (id, key, text, long_text, blob) = (t.id, t.key, t.text, t.long_text, t.blob);
        let statements = [
            format!(
                "CREATE TABLE companies (id {id}, name {text} NOT NULL, token_hash {key} NOT NULL UNIQUE, created {int} NOT NULL)"
            ),
            format!(
                "CREATE TABLE clients (install_id {key} NOT NULL, company_id {int} NOT NULL, hostname {text} NOT NULL, app {text} NOT NULL, version {text} NOT NULL, os {text} NOT NULL, package {text} NOT NULL, pbx {text} NOT NULL, pbx_version {text} NOT NULL, first_seen {int} NOT NULL, last_seen {int} NOT NULL, last_event {text} NOT NULL, heartbeat {int} NOT NULL, pbx_connected {int} NOT NULL, chat_connected {int} NOT NULL, softphone_registered {int} NOT NULL, softphone_enabled {int} NOT NULL, audio_ok {int} NOT NULL, uptime_secs {int} NOT NULL, last_error {int} NOT NULL, PRIMARY KEY (company_id, install_id))"
            ),
            format!(
                "CREATE TABLE reports (id {key} NOT NULL PRIMARY KEY, company_id {int} NOT NULL, install_id {key} NOT NULL, reason {text} NOT NULL, time {int} NOT NULL, received {int} NOT NULL, hostname {text} NOT NULL, app {text} NOT NULL, version {text} NOT NULL, os {text} NOT NULL, package {text} NOT NULL, pbx {text} NOT NULL, pbx_version {text} NOT NULL, comment {long_text} NOT NULL, summary {long_text} NOT NULL, log_size {int} NOT NULL, log_gz {blob}, done {int} NOT NULL, note {long_text} NOT NULL, issue_url {text} NOT NULL)"
            ),
            "CREATE INDEX reports_company_time ON reports (company_id, received)".to_string(),
            format!(
                "CREATE TABLE errors (company_id {int} NOT NULL, day {int} NOT NULL, app {key} NOT NULL, version {key} NOT NULL, kind {key} NOT NULL, count {int} NOT NULL, PRIMARY KEY (company_id, day, app, version, kind))"
            ),
            format!(
                "CREATE TABLE users (id {id}, name {key} NOT NULL UNIQUE, pw_hash {text} NOT NULL, role {text} NOT NULL, company_id {int} NOT NULL, created {int} NOT NULL)"
            ),
            format!(
                "CREATE TABLE sessions (token_hash {key} NOT NULL PRIMARY KEY, user_id {int} NOT NULL, expires {int} NOT NULL)"
            ),
            format!(
                "CREATE TABLE settings (name {key} NOT NULL PRIMARY KEY, value {long_text} NOT NULL)"
            ),
        ];
        let mut tx = self.pool.begin().await?;
        for s in &statements {
            sqlx::query(s)
                .execute(&mut *tx)
                .await
                .with_context(|| s.clone())?;
        }
        sqlx::query("DELETE FROM schema_version")
            .execute(&mut *tx)
            .await?;
        sqlx::query(&self.sql("INSERT INTO schema_version (version) VALUES (?)"))
            .bind(SCHEMA)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        tracing::info!(version = SCHEMA, "Datenbank eingerichtet");
        Ok(())
    }

    // -- Firmen -------------------------------------------------------------

    pub async fn add_company(&self, name: &str, token_hash: &str) -> anyhow::Result<()> {
        sqlx::query(
            &self.sql("INSERT INTO companies (name, token_hash, created) VALUES (?, ?, ?)"),
        )
        .bind(name)
        .bind(token_hash)
        .bind(now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn company_by_token(&self, token_hash: &str) -> anyhow::Result<Option<i64>> {
        Ok(
            sqlx::query(&self.sql("SELECT id FROM companies WHERE token_hash = ?"))
                .bind(token_hash)
                .fetch_optional(&self.pool)
                .await?
                .map(|r| r.get::<i64, _>(0)),
        )
    }

    pub async fn companies(&self) -> anyhow::Result<Vec<(i64, String)>> {
        Ok(sqlx::query("SELECT id, name FROM companies ORDER BY name")
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| (r.get::<i64, _>(0), r.get::<String, _>(1)))
            .collect())
    }

    pub async fn set_company_token(&self, name: &str, token_hash: &str) -> anyhow::Result<bool> {
        let r = sqlx::query(&self.sql("UPDATE companies SET token_hash = ? WHERE name = ?"))
            .bind(token_hash)
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(r.rows_affected() > 0)
    }

    // -- Statusmeldungen ----------------------------------------------------

    pub async fn heartbeat(&self, company: i64, hb: &Heartbeat) -> anyhow::Result<()> {
        let Some(c) = &hb.client else {
            anyhow::bail!("Statusmeldung ohne Angaben zum Arbeitsplatz");
        };
        let now = now();
        let exists =
            sqlx::query(&self.sql("SELECT 1 FROM clients WHERE company_id = ? AND install_id = ?"))
                .bind(company)
                .bind(&c.install_id)
                .fetch_optional(&self.pool)
                .await?
                .is_some();
        let q = if exists {
            "UPDATE clients SET hostname = ?, app = ?, version = ?, os = ?, package = ?, pbx = ?, pbx_version = ?, last_seen = ?, last_event = ?, heartbeat = 1, pbx_connected = ?, chat_connected = ?, softphone_registered = ?, softphone_enabled = ?, audio_ok = ?, uptime_secs = ?, last_error = ? WHERE company_id = ? AND install_id = ?"
        } else {
            "INSERT INTO clients (hostname, app, version, os, package, pbx, pbx_version, last_seen, last_event, heartbeat, pbx_connected, chat_connected, softphone_registered, softphone_enabled, audio_ok, uptime_secs, last_error, company_id, install_id, first_seen) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
        };
        let sql = self.sql(q);
        let mut query = sqlx::query(&sql)
            .bind(cut(&c.hostname, 255))
            .bind(cut(&c.app, 64))
            .bind(cut(&c.version, 64))
            .bind(cut(&c.os, 1000))
            .bind(cut(&c.package, 32))
            .bind(cut(&c.pbx, 255))
            .bind(cut(&c.pbx_version, 64))
            .bind(now)
            .bind(cut(&hb.event, 32))
            .bind(i64::from(hb.pbx_connected))
            .bind(i64::from(hb.chat_connected))
            .bind(i64::from(hb.softphone_registered))
            .bind(i64::from(hb.softphone_enabled))
            .bind(i64::from(hb.audio_ok))
            .bind(hb.uptime_secs.min(i64::MAX as u64) as i64)
            .bind(hb.last_error.min(i64::MAX as u64) as i64)
            .bind(company)
            .bind(cut(&c.install_id, 64));
        if !exists {
            query = query.bind(now);
        }
        query.execute(&self.pool).await?;
        let day = now / 86_400;
        for (kind, count) in &hb.errors {
            self.count_error(company, day, &c.app, &c.version, kind, i64::from(*count))
                .await?;
        }
        Ok(())
    }

    async fn count_error(
        &self,
        company: i64,
        day: i64,
        app: &str,
        version: &str,
        kind: &str,
        count: i64,
    ) -> anyhow::Result<()> {
        if count <= 0 {
            return Ok(());
        }
        let (app, version, kind) = (cut(app, 64), cut(version, 64), cut(kind, 32));
        let r = sqlx::query(&self.sql(
            "UPDATE errors SET count = count + ? WHERE company_id = ? AND day = ? AND app = ? AND version = ? AND kind = ?",
        ))
        .bind(count)
        .bind(company)
        .bind(day)
        .bind(app)
        .bind(version)
        .bind(kind)
        .execute(&self.pool)
        .await?;
        if r.rows_affected() == 0 {
            sqlx::query(&self.sql(
                "INSERT INTO errors (company_id, day, app, version, kind, count) VALUES (?, ?, ?, ?, ?, ?)",
            ))
            .bind(company)
            .bind(day)
            .bind(app)
            .bind(version)
            .bind(kind)
            .bind(count)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // -- Berichte -----------------------------------------------------------

    /// Legt einen Bericht an. `log_gz` nur, wenn die Berichte in der
    /// Datenbank liegen. `false`, wenn die Nummer schon vergeben ist.
    pub async fn add_report(
        &self,
        company: i64,
        r: &Report,
        log_size: usize,
        log_gz: Option<Vec<u8>>,
    ) -> anyhow::Result<bool> {
        let exists = sqlx::query(&self.sql("SELECT 1 FROM reports WHERE id = ?"))
            .bind(&r.id)
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        if exists {
            return Ok(false);
        }
        let c = &r.client;
        sqlx::query(&self.sql(
            "INSERT INTO reports (id, company_id, install_id, reason, time, received, hostname, app, version, os, package, pbx, pbx_version, comment, summary, log_size, log_gz, done, note, issue_url) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0, '', '')",
        ))
        .bind(&r.id)
        .bind(company)
        .bind(cut(&c.install_id, 64))
        .bind(cut(&r.reason, 32))
        .bind(r.time.min(i64::MAX as u64) as i64)
        .bind(now())
        .bind(cut(&c.hostname, 255))
        .bind(cut(&c.app, 64))
        .bind(cut(&c.version, 64))
        .bind(cut(&c.os, 1000))
        .bind(cut(&c.package, 32))
        .bind(cut(&c.pbx, 255))
        .bind(cut(&c.pbx_version, 64))
        .bind(cut(&r.comment, 20_000))
        .bind(cut(&r.summary, 2_000))
        .bind(log_size as i64)
        .bind(log_gz)
        .execute(&self.pool)
        .await?;
        // Auch ohne Statusmeldung erscheint der Arbeitsplatz in der Liste
        let known =
            sqlx::query(&self.sql("SELECT 1 FROM clients WHERE company_id = ? AND install_id = ?"))
                .bind(company)
                .bind(&c.install_id)
                .fetch_optional(&self.pool)
                .await?
                .is_some();
        if !known {
            let hb = Heartbeat {
                client: Some(c.clone()),
                event: "report".into(),
                ..Default::default()
            };
            self.heartbeat(company, &hb).await?;
            sqlx::query(
                &self.sql(
                    "UPDATE clients SET heartbeat = 0 WHERE company_id = ? AND install_id = ?",
                ),
            )
            .bind(company)
            .bind(&c.install_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(true)
    }

    pub async fn report_log(&self, id: &str) -> anyhow::Result<Option<Vec<u8>>> {
        Ok(
            sqlx::query(&self.sql("SELECT log_gz FROM reports WHERE id = ?"))
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .and_then(|r| r.try_get::<Option<Vec<u8>>, _>(0).ok().flatten()),
        )
    }

    pub async fn reports(&self, filter: &ReportFilter) -> anyhow::Result<Vec<ReportRow>> {
        let mut q = self.report_select();
        q.push_str(" WHERE 1 = 1");
        if filter.company.is_some() {
            q.push_str(" AND company_id = ?");
        }
        if filter.install_id.is_some() {
            q.push_str(" AND install_id = ?");
        }
        if filter.reason.is_some() {
            q.push_str(" AND reason = ?");
        }
        if filter.open_only {
            q.push_str(" AND done = 0");
        }
        if filter.search.is_some() {
            q.push_str(" AND (id = ? OR hostname LIKE ? OR version = ?)");
        }
        q.push_str(" ORDER BY received DESC LIMIT ");
        q.push_str(&filter.limit.clamp(1, 1000).to_string());
        let sql = self.sql(&q);
        let mut query = sqlx::query(&sql);
        if let Some(c) = filter.company {
            query = query.bind(c);
        }
        if let Some(i) = &filter.install_id {
            query = query.bind(i);
        }
        if let Some(r) = &filter.reason {
            query = query.bind(r);
        }
        if let Some(s) = &filter.search {
            let s = s.trim().to_string();
            query = query.bind(s.to_uppercase()).bind(format!("%{s}%")).bind(s);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows.iter().map(ReportRow::from_row).collect())
    }

    pub async fn report(&self, id: &str) -> anyhow::Result<Option<ReportRow>> {
        Ok(
            sqlx::query(&self.sql(&format!("{} WHERE id = ?", self.report_select())))
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .as_ref()
                .map(ReportRow::from_row),
        )
    }

    pub async fn set_report_state(&self, id: &str, done: bool, note: &str) -> anyhow::Result<()> {
        sqlx::query(&self.sql("UPDATE reports SET done = ?, note = ? WHERE id = ?"))
            .bind(i64::from(done))
            .bind(cut(note, 20_000))
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn set_issue_url(&self, id: &str, url: &str) -> anyhow::Result<()> {
        sqlx::query(&self.sql("UPDATE reports SET issue_url = ? WHERE id = ?"))
            .bind(cut(url, 1000))
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Löscht alte Berichte, alte Fehlerzähler und abgelaufene Anmeldungen.
    /// Liefert die IDs der gelöschten Berichte (für die Dateien).
    pub async fn cleanup(&self, retention_days: u32) -> anyhow::Result<Vec<(i64, String)>> {
        let cutoff = now() - i64::from(retention_days) * 86_400;
        let old: Vec<(i64, String)> =
            sqlx::query(&self.sql("SELECT company_id, id FROM reports WHERE received < ?"))
                .bind(cutoff)
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| (r.get::<i64, _>(0), r.get::<String, _>(1)))
                .collect();
        sqlx::query(&self.sql("DELETE FROM reports WHERE received < ?"))
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        sqlx::query(&self.sql("DELETE FROM errors WHERE day < ?"))
            .bind(cutoff / 86_400)
            .execute(&self.pool)
            .await?;
        // Arbeitsplätze ohne Meldung seit der Aufbewahrungszeit
        sqlx::query(&self.sql("DELETE FROM clients WHERE last_seen < ?"))
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        sqlx::query(&self.sql("DELETE FROM sessions WHERE expires < ?"))
            .bind(now())
            .execute(&self.pool)
            .await?;
        Ok(old)
    }

    // -- Arbeitsplätze ------------------------------------------------------

    pub async fn clients(&self, company: Option<i64>) -> anyhow::Result<Vec<ClientRow>> {
        let mut q = String::from(CLIENT_SELECT);
        if company.is_some() {
            q.push_str(" WHERE c.company_id = ?");
        }
        q.push_str(" ORDER BY c.hostname, c.app");
        let sql = self.sql(&q);
        let mut query = sqlx::query(&sql).bind(now() - 86_400);
        if let Some(c) = company {
            query = query.bind(c);
        }
        Ok(query
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(ClientRow::from_row)
            .collect())
    }

    pub async fn client(
        &self,
        company: i64,
        install_id: &str,
    ) -> anyhow::Result<Option<ClientRow>> {
        let sql = self.sql(&format!(
            "{CLIENT_SELECT} WHERE c.company_id = ? AND c.install_id = ?"
        ));
        Ok(sqlx::query(&sql)
            .bind(now() - 86_400)
            .bind(company)
            .bind(install_id)
            .fetch_optional(&self.pool)
            .await?
            .as_ref()
            .map(ClientRow::from_row))
    }

    /// Fehler der letzten `days` Tage: (Tag, App, Version, Art, Anzahl)
    pub async fn errors(
        &self,
        company: Option<i64>,
        days: i64,
    ) -> anyhow::Result<Vec<(i64, String, String, String, i64)>> {
        let mut q = format!(
            "SELECT day, app, version, kind, {} FROM errors WHERE day >= ?",
            self.int_expr("SUM(count)")
        );
        if company.is_some() {
            q.push_str(" AND company_id = ?");
        }
        q.push_str(" GROUP BY day, app, version, kind ORDER BY day");
        let sql = self.sql(&q);
        let mut query = sqlx::query(&sql).bind(now() / 86_400 - days + 1);
        if let Some(c) = company {
            query = query.bind(c);
        }
        Ok(query
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| {
                (
                    r.get::<i64, _>(0),
                    r.get::<String, _>(1),
                    r.get::<String, _>(2),
                    r.get::<String, _>(3),
                    get_i64(&r, 4),
                )
            })
            .collect())
    }

    /// Berichte je App und Anlass (für Metriken)
    pub async fn report_counts(&self) -> anyhow::Result<Vec<(String, String, i64)>> {
        Ok(
            sqlx::query("SELECT app, reason, COUNT(*) FROM reports GROUP BY app, reason")
                .fetch_all(&self.pool)
                .await?
                .into_iter()
                .map(|r| (r.get::<String, _>(0), r.get::<String, _>(1), get_i64(&r, 2)))
                .collect(),
        )
    }

    // -- Benutzer und Anmeldung --------------------------------------------

    pub async fn add_user(
        &self,
        name: &str,
        pw_hash: &str,
        role: &str,
        company: i64,
    ) -> anyhow::Result<()> {
        let exists = sqlx::query(&self.sql("SELECT 1 FROM users WHERE name = ?"))
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
            .is_some();
        let q = if exists {
            "UPDATE users SET pw_hash = ?, role = ?, company_id = ? WHERE name = ?"
        } else {
            "INSERT INTO users (pw_hash, role, company_id, name, created) VALUES (?, ?, ?, ?, ?)"
        };
        let sql = self.sql(q);
        let mut query = sqlx::query(&sql)
            .bind(pw_hash)
            .bind(role)
            .bind(company)
            .bind(name);
        if !exists {
            query = query.bind(now());
        }
        query.execute(&self.pool).await?;
        Ok(())
    }

    pub async fn user_count(&self) -> anyhow::Result<i64> {
        Ok(get_i64(
            &sqlx::query("SELECT COUNT(*) FROM users")
                .fetch_one(&self.pool)
                .await?,
            0,
        ))
    }

    /// (id, pw_hash)
    pub async fn user_login(&self, name: &str) -> anyhow::Result<Option<(i64, String)>> {
        Ok(
            sqlx::query(&self.sql("SELECT id, pw_hash FROM users WHERE name = ?"))
                .bind(name)
                .fetch_optional(&self.pool)
                .await?
                .map(|r| (r.get::<i64, _>(0), r.get::<String, _>(1))),
        )
    }

    pub async fn add_session(
        &self,
        token_hash: &str,
        user: i64,
        expires: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            &self.sql("INSERT INTO sessions (token_hash, user_id, expires) VALUES (?, ?, ?)"),
        )
        .bind(token_hash)
        .bind(user)
        .bind(expires)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_session(&self, token_hash: &str) -> anyhow::Result<()> {
        sqlx::query(&self.sql("DELETE FROM sessions WHERE token_hash = ?"))
            .bind(token_hash)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn session_user(&self, token_hash: &str) -> anyhow::Result<Option<User>> {
        Ok(sqlx::query(&self.sql(
            "SELECT u.id, u.name, u.role, u.company_id FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ? AND s.expires > ?",
        ))
        .bind(token_hash)
        .bind(now())
        .fetch_optional(&self.pool)
        .await?
        .map(|r| User {
            id: r.get(0),
            name: r.get(1),
            role: r.get(2),
            company: r.get(3),
        }))
    }

    // -- Einstellungen (z. B. neueste Versionen) ---------------------------

    pub async fn setting(&self, name: &str) -> anyhow::Result<Option<String>> {
        let sql = self.sql(&format!(
            "SELECT {} FROM settings WHERE name = ?",
            self.text_col("value")
        ));
        Ok(sqlx::query(&sql)
            .bind(name)
            .fetch_optional(&self.pool)
            .await?
            .map(|r| get_text(&r, 0)))
    }

    pub async fn set_setting(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let r = sqlx::query(&self.sql("UPDATE settings SET value = ? WHERE name = ?"))
            .bind(value)
            .bind(name)
            .execute(&self.pool)
            .await?;
        if r.rows_affected() == 0 && self.setting(name).await?.is_none() {
            sqlx::query(&self.sql("INSERT INTO settings (name, value) VALUES (?, ?)"))
                .bind(name)
                .bind(value)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub async fn ping(&self) -> bool {
        sqlx::query("SELECT 1").fetch_one(&self.pool).await.is_ok()
    }
}

/// Text, auch wenn MariaDB ihn über `sqlx::Any` als BLOB liefert
fn get_text(r: &AnyRow, i: usize) -> String {
    r.try_get::<String, _>(i)
        .or_else(|_| {
            r.try_get::<Vec<u8>, _>(i)
                .map(|b| String::from_utf8_lossy(&b).into_owned())
        })
        .unwrap_or_default()
}

/// COUNT/SUM liefern je nach Datenbank unterschiedliche Typen
fn get_i64(r: &AnyRow, i: usize) -> i64 {
    r.try_get::<i64, _>(i)
        .or_else(|_| r.try_get::<i32, _>(i).map(i64::from))
        .or_else(|_| r.try_get::<f64, _>(i).map(|f| f as i64))
        .or_else(|_| {
            r.try_get::<String, _>(i)
                .map(|s| s.parse::<f64>().unwrap_or(0.0) as i64)
        })
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    /// "support" (darf Issues erstellen, Berichte erledigen) oder "lesen"
    pub role: String,
    /// 0 = alle Firmen
    pub company: i64,
}

impl User {
    pub fn company_filter(&self) -> Option<i64> {
        (self.company != 0).then_some(self.company)
    }

    pub fn may_edit(&self) -> bool {
        self.role == "support" || self.role == "admin"
    }
}

#[derive(Debug, Default)]
pub struct ReportFilter {
    pub company: Option<i64>,
    pub install_id: Option<String>,
    pub reason: Option<String>,
    pub search: Option<String>,
    pub open_only: bool,
    pub limit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ReportRow {
    pub id: String,
    pub company: i64,
    pub install_id: String,
    pub reason: String,
    pub time: i64,
    pub received: i64,
    pub hostname: String,
    pub app: String,
    pub version: String,
    pub os: String,
    pub package: String,
    pub pbx: String,
    pub pbx_version: String,
    pub comment: String,
    pub summary: String,
    pub log_size: i64,
    pub done: bool,
    pub note: String,
    pub issue_url: String,
}

impl ReportRow {
    fn from_row(r: &AnyRow) -> Self {
        ReportRow {
            id: r.get(0),
            company: r.get(1),
            install_id: r.get(2),
            reason: r.get(3),
            time: r.get(4),
            received: r.get(5),
            hostname: r.get(6),
            app: r.get(7),
            version: r.get(8),
            os: r.get(9),
            package: r.get(10),
            pbx: r.get(11),
            pbx_version: r.get(12),
            comment: get_text(r, 13),
            summary: get_text(r, 14),
            log_size: r.get(15),
            done: r.get::<i64, _>(16) != 0,
            note: get_text(r, 17),
            issue_url: r.get(18),
        }
    }
}

const CLIENT_SELECT: &str = "SELECT c.install_id, c.company_id, c.hostname, c.app, c.version, c.os, c.package, c.pbx, c.pbx_version, c.first_seen, c.last_seen, c.last_event, c.heartbeat, c.pbx_connected, c.chat_connected, c.softphone_registered, c.softphone_enabled, c.audio_ok, c.uptime_secs, c.last_error, (SELECT COUNT(*) FROM reports r WHERE r.company_id = c.company_id AND r.install_id = c.install_id AND r.received >= ?) FROM clients c";

#[derive(Debug, Clone, Serialize)]
pub struct ClientRow {
    pub install_id: String,
    pub company: i64,
    pub hostname: String,
    pub app: String,
    pub version: String,
    pub os: String,
    pub package: String,
    pub pbx: String,
    pub pbx_version: String,
    pub first_seen: i64,
    pub last_seen: i64,
    pub last_event: String,
    /// Schickt Statusmeldungen (sonst nur Berichte)
    pub heartbeat: bool,
    pub pbx_connected: bool,
    pub chat_connected: bool,
    pub softphone_registered: bool,
    pub softphone_enabled: bool,
    pub audio_ok: bool,
    pub uptime_secs: i64,
    pub last_error: i64,
    /// Berichte der letzten 24 Stunden
    pub reports_24h: i64,
}

impl ClientRow {
    fn from_row(r: &AnyRow) -> Self {
        let b = |i: usize| r.get::<i64, _>(i) != 0;
        ClientRow {
            install_id: r.get(0),
            company: r.get(1),
            hostname: r.get(2),
            app: r.get(3),
            version: r.get(4),
            os: r.get(5),
            package: r.get(6),
            pbx: r.get(7),
            pbx_version: r.get(8),
            first_seen: r.get(9),
            last_seen: r.get(10),
            last_event: r.get(11),
            heartbeat: b(12),
            pbx_connected: b(13),
            chat_connected: b(14),
            softphone_registered: b(15),
            softphone_enabled: b(16),
            audio_ok: b(17),
            uptime_secs: r.get(18),
            last_error: r.get(19),
            reports_24h: get_i64(r, 20),
        }
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Kürzt auf `max` Bytes an einer Zeichengrenze
pub fn cut(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Fehler je Art zusammengezählt (für die Übersicht)
pub fn sum_by_kind(errors: &[(i64, String, String, String, i64)]) -> BTreeMap<String, i64> {
    let mut out = BTreeMap::new();
    for (_, _, _, kind, n) in errors {
        *out.entry(kind.clone()).or_default() += n;
    }
    out
}
