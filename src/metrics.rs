//! Kennzahlen für das Monitoring (Prometheus-Textformat).
//!
//! Nur Zustand von Programmen, nie Personen: standardmässig zusammengezählt
//! je App, Version und Zustand. Mit `metrics_per_host = true` zusätzlich ein
//! Wert je Rechner (0/1), ohne Benutzer, Präsenz oder Gespräche.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::db::{ClientRow, now};
use crate::{App, auth, releases};

/// Zustand eines Arbeitsplatzes für Übersicht und Metriken
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Health {
    Ok,
    Fehler,
    NichtErreichbar,
    /// Schickt keine Statusmeldungen, nur Berichte
    Unbekannt,
}

impl Health {
    pub fn of(c: &ClientRow, offline_after_minutes: u32) -> Self {
        if !c.heartbeat {
            return Health::Unbekannt;
        }
        if c.last_event == "stop" || now() - c.last_seen > i64::from(offline_after_minutes) * 60 {
            return Health::NichtErreichbar;
        }
        let phone_bad = c.softphone_enabled && !c.softphone_registered;
        if !c.pbx_connected || phone_bad || !c.audio_ok || c.reports_24h > 0 {
            Health::Fehler
        } else {
            Health::Ok
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Health::Ok => "ok",
            Health::Fehler => "fehler",
            Health::NichtErreichbar => "nicht_erreichbar",
            Health::Unbekannt => "unbekannt",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Health::Ok => "In Ordnung",
            Health::Fehler => "Fehler",
            Health::NichtErreichbar => "Nicht erreichbar",
            Health::Unbekannt => "Nur Berichte",
        }
    }
}

/// Wert für ein Label im Prometheus-Format
fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub async fn metrics(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let token = &app.config.metrics_token;
    if !token.is_empty() && !auth::bearer(&headers).is_some_and(|t| auth::same(t, token)) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let db_ok = app.db.ping().await;
    let clients = app.db.clients(None).await.unwrap_or_default();
    let mut out = String::new();
    let offline = app.config.offline_after_minutes;

    let mut by_state: BTreeMap<(String, String, &str), u64> = BTreeMap::new();
    let mut registered: BTreeMap<String, u64> = BTreeMap::new();
    let mut pbx: BTreeMap<String, u64> = BTreeMap::new();
    let mut chat: BTreeMap<String, u64> = BTreeMap::new();
    let mut outdated: BTreeMap<String, u64> = BTreeMap::new();
    for c in &clients {
        let h = Health::of(c, offline);
        *by_state
            .entry((c.app.clone(), c.version.clone(), h.key()))
            .or_default() += 1;
        if h == Health::NichtErreichbar || h == Health::Unbekannt {
            continue;
        }
        *registered.entry(c.app.clone()).or_default() += u64::from(c.softphone_registered);
        *pbx.entry(c.app.clone()).or_default() += u64::from(c.pbx_connected);
        *chat.entry(c.app.clone()).or_default() += u64::from(c.chat_connected);
        if app
            .latest(&c.app)
            .is_some_and(|l| releases::older(&c.version, &l))
        {
            *outdated.entry(c.app.clone()).or_default() += 1;
        }
    }

    let _ = writeln!(
        out,
        "# HELP stardash_up StarDash läuft und erreicht die Datenbank"
    );
    let _ = writeln!(out, "# TYPE stardash_up gauge");
    let _ = writeln!(out, "stardash_up {}", u8::from(db_ok));
    let _ = writeln!(out, "# HELP stardash_info Version von StarDash");
    let _ = writeln!(out, "# TYPE stardash_info gauge");
    let _ = writeln!(out, "stardash_info{{version=\"{}\"}} 1", crate::VERSION);

    let _ = writeln!(
        out,
        "# HELP stardash_clients Arbeitsplätze je App, Version und Zustand"
    );
    let _ = writeln!(out, "# TYPE stardash_clients gauge");
    for ((a, v, s), n) in &by_state {
        let _ = writeln!(
            out,
            "stardash_clients{{app=\"{}\",version=\"{}\",zustand=\"{s}\"}} {n}",
            esc(a),
            esc(v)
        );
    }
    for (name, help, map) in [
        (
            "stardash_clients_softphone_registriert",
            "Erreichbare Arbeitsplätze mit angemeldetem Softphone",
            &registered,
        ),
        (
            "stardash_clients_anlage_verbunden",
            "Erreichbare Arbeitsplätze mit Verbindung zur Anlage",
            &pbx,
        ),
        (
            "stardash_clients_chat_verbunden",
            "Erreichbare Arbeitsplätze mit verbundenem Chat",
            &chat,
        ),
        (
            "stardash_clients_veraltet",
            "Erreichbare Arbeitsplätze mit älterer als der neuesten Version",
            &outdated,
        ),
    ] {
        let _ = writeln!(out, "# HELP {name} {help}");
        let _ = writeln!(out, "# TYPE {name} gauge");
        for (a, n) in map {
            let _ = writeln!(out, "{name}{{app=\"{}\"}} {n}", esc(a));
        }
    }

    let _ = writeln!(
        out,
        "# HELP stardash_berichte Gespeicherte Berichte je App und Anlass"
    );
    let _ = writeln!(out, "# TYPE stardash_berichte gauge");
    for (a, r, n) in app.db.report_counts().await.unwrap_or_default() {
        let _ = writeln!(
            out,
            "stardash_berichte{{app=\"{}\",anlass=\"{}\"}} {n}",
            esc(&a),
            esc(&r)
        );
    }

    let _ = writeln!(
        out,
        "# HELP stardash_fehler_heute Gemeldete Fehler heute je App, Version und Art"
    );
    let _ = writeln!(out, "# TYPE stardash_fehler_heute gauge");
    let mut today: BTreeMap<(String, String, String), i64> = BTreeMap::new();
    for (_, a, v, k, n) in app.db.errors(None, 1).await.unwrap_or_default() {
        *today.entry((a, v, k)).or_default() += n;
    }
    for ((a, v, k), n) in today {
        let _ = writeln!(
            out,
            "stardash_fehler_heute{{app=\"{}\",version=\"{}\",art=\"{}\"}} {n}",
            esc(&a),
            esc(&v),
            esc(&k)
        );
    }

    if app.config.metrics_per_host {
        let _ = writeln!(
            out,
            "# HELP stardash_client_ok Zustand eines Rechners (1 = in Ordnung)"
        );
        let _ = writeln!(out, "# TYPE stardash_client_ok gauge");
        for c in &clients {
            let ok = Health::of(c, offline) == Health::Ok;
            let _ = writeln!(
                out,
                "stardash_client_ok{{rechner=\"{}\",app=\"{}\"}} {}",
                esc(&c.hostname),
                esc(&c.app),
                u8::from(ok)
            );
        }
    }

    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        out,
    )
        .into_response()
}
