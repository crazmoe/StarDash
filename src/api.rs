//! Schnittstelle für die Clients (v1): Berichte und Statusmeldungen.

use std::io::Read;
use std::sync::Arc;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};

use crate::App;
use crate::auth;
use crate::config::ReportStore;
use crate::model::{Heartbeat, Report, valid_report_id};

/// Obergrenze nach dem Entpacken
const MAX_UNPACKED: u64 = 64 * 1024 * 1024;

fn error(status: StatusCode, msg: &str) -> Response {
    (status, Json(serde_json::json!({ "error": msg }))).into_response()
}

async fn company(app: &App, headers: &HeaderMap) -> Result<i64, Response> {
    let Some(token) = auth::bearer(headers) else {
        return Err(error(StatusCode::UNAUTHORIZED, "Firmen-Schlüssel fehlt"));
    };
    match app.db.company_by_token(&auth::hash_token(token)).await {
        Ok(Some(id)) => Ok(id),
        Ok(None) => Err(error(
            StatusCode::UNAUTHORIZED,
            "Firmen-Schlüssel unbekannt",
        )),
        Err(e) => {
            tracing::error!(error = %e, "Datenbank");
            Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Datenbank nicht erreichbar",
            ))
        }
    }
}

/// Entpackt den Körper, falls gzip
fn body(headers: &HeaderMap, body: &[u8]) -> Result<Vec<u8>, Response> {
    let gzip = headers
        .get(header::CONTENT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("gzip"));
    if !gzip {
        return Ok(body.to_vec());
    }
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(body)
        .take(MAX_UNPACKED + 1)
        .read_to_end(&mut out)
        .map_err(|_| error(StatusCode::BAD_REQUEST, "gzip ungültig"))?;
    if out.len() as u64 > MAX_UNPACKED {
        return Err(error(StatusCode::PAYLOAD_TOO_LARGE, "Bericht zu gross"));
    }
    Ok(out)
}

pub fn pack(text: &str) -> std::io::Result<Vec<u8>> {
    use std::io::Write;
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(text.as_bytes())?;
    gz.finish()
}

pub async fn report(State(app): State<Arc<App>>, headers: HeaderMap, raw: Bytes) -> Response {
    let company = match company(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let data = match body(&headers, &raw) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let report: Report = match serde_json::from_slice(&data) {
        Ok(r) => r,
        Err(e) => return error(StatusCode::BAD_REQUEST, &format!("JSON ungültig: {e}")),
    };
    if !valid_report_id(&report.id) {
        return error(StatusCode::BAD_REQUEST, "Bericht-Nr. ungültig");
    }
    let packed = match pack(&report.log) {
        Ok(p) => p,
        Err(e) => return error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let in_db = app.config.database.reports == ReportStore::Datenbank;
    if !in_db {
        let path = app.report_path(company, &report.id);
        let write = async {
            if let Some(dir) = path.parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            tokio::fs::write(&path, &packed).await
        };
        if let Err(e) = write.await {
            tracing::error!(error = %e, path = %path.display(), "Bericht nicht gespeichert");
            return error(
                StatusCode::INSUFFICIENT_STORAGE,
                "Bericht nicht gespeichert",
            );
        }
    }
    match app
        .db
        .add_report(company, &report, report.log.len(), in_db.then_some(packed))
        .await
    {
        Ok(true) => {
            tracing::info!(
                id = report.id,
                reason = report.reason,
                host = report.client.hostname,
                "Bericht erhalten"
            );
        }
        // Schon da: Client hat nach einem Zeitüberlauf noch einmal gesendet
        Ok(false) => {}
        Err(e) => {
            tracing::error!(error = %e, "Bericht nicht gespeichert");
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Datenbank nicht erreichbar",
            );
        }
    }
    (StatusCode::OK, Json(serde_json::json!({ "id": report.id }))).into_response()
}

pub async fn heartbeat(State(app): State<Arc<App>>, headers: HeaderMap, raw: Bytes) -> Response {
    let company = match company(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let data = match body(&headers, &raw) {
        Ok(d) => d,
        Err(r) => return r,
    };
    let hb: Heartbeat = match serde_json::from_slice(&data) {
        Ok(h) => h,
        Err(e) => return error(StatusCode::BAD_REQUEST, &format!("JSON ungültig: {e}")),
    };
    if hb.client.as_ref().is_none_or(|c| c.install_id.is_empty()) {
        return error(StatusCode::BAD_REQUEST, "Angaben zum Arbeitsplatz fehlen");
    }
    match app.db.heartbeat(company, &hb).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Statusmeldung nicht gespeichert");
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "Datenbank nicht erreichbar",
            )
        }
    }
}

pub async fn health(State(app): State<Arc<App>>) -> Response {
    if app.db.ping().await {
        (StatusCode::OK, "ok").into_response()
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Datenbank nicht erreichbar",
        )
            .into_response()
    }
}
