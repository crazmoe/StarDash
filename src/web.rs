//! Weboberfläche: Anmeldung, Übersicht, Arbeitsplätze, Berichte, Issues.
//!
//! Bewusst schlicht: HTML vom Server, kein JavaScript nötig. Die Seiten
//! zeigen den Zustand der Programme, nie das Verhalten von Personen.

use std::collections::BTreeMap;
use std::fmt::Write;
use std::io::Read;
use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use serde::Deserialize;

use crate::db::{ClientRow, ReportFilter, ReportRow, User, now};
use crate::issues::{self, Created};
use crate::metrics::Health;
use crate::model::{REASONS, reason_label, valid_report_id};
use crate::{App, VERSION, auth, releases};

const COOKIE: &str = "stardash_session";
/// Anmeldung gilt so lange
const SESSION_SECS: i64 = 12 * 3600;

pub fn routes() -> Router<Arc<App>> {
    Router::new()
        .route("/", get(overview))
        .route("/login", get(login_form).post(login))
        .route("/logout", post(logout))
        .route("/arbeitsplaetze", get(clients))
        .route("/arbeitsplaetze/{company}/{install}", get(client))
        .route("/berichte", get(reports))
        .route("/berichte/{id}", get(report))
        .route("/berichte/{id}/protokoll", get(report_log))
        .route("/berichte/{id}/status", post(report_state))
        .route("/berichte/{id}/issue", get(issue_form).post(issue_create))
}

// ---------------------------------------------------------------------------
// Hilfen

/// Text für HTML
fn e(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Zeitpunkt als Text (UTC, der Browser rechnet nicht um)
fn when(t: i64) -> String {
    if t <= 0 {
        return "–".into();
    }
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    // Tage seit 1970 → Datum (Algorithmus von H. Hinnant)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{d:02}.{m:02}.{y} {:02}:{:02} UTC",
        secs / 3600,
        secs % 3600 / 60
    )
}

fn ago(t: i64) -> String {
    let d = now() - t;
    if t <= 0 {
        "–".into()
    } else if d < 90 {
        "gerade eben".into()
    } else if d < 3600 {
        format!("vor {} min", d / 60)
    } else if d < 86_400 * 2 {
        format!("vor {} h", d / 3600)
    } else {
        format!("vor {} Tagen", d / 86_400)
    }
}

fn size(n: i64) -> String {
    if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / 1_048_576.0)
    } else {
        format!("{} KB", (n + 1023) / 1024)
    }
}

const CSS: &str = r#"
:root { --bg:#f6f7f9; --panel:#fff; --text:#1d2430; --muted:#5e6878; --line:#dde1e7; --accent:#0b6bcb; --ok:#1a7f37; --warn:#b26a00; --bad:#c62828; --off:#7a828e; }
@media (prefers-color-scheme: dark) { :root { --bg:#14171c; --panel:#1d2128; --text:#e6e9ee; --muted:#9aa3b0; --line:#2e343d; --accent:#5aa9ff; --ok:#4cc26a; --warn:#e3a33a; --bad:#ff6b6b; --off:#8b939e; } }
* { box-sizing: border-box; }
body { margin:0; font:15px/1.45 system-ui, -apple-system, "Segoe UI", sans-serif; background:var(--bg); color:var(--text); }
header { display:flex; flex-wrap:wrap; align-items:center; gap:1rem; padding:.6rem 1rem; background:var(--panel); border-bottom:1px solid var(--line); }
header b { font-size:1.1rem; }
header nav { display:flex; gap:.9rem; flex:1; flex-wrap:wrap; }
header form { margin:0; }
a { color:var(--accent); text-decoration:none; } a:hover { text-decoration:underline; }
main { max-width:1200px; margin:0 auto; padding:1rem; }
h1 { font-size:1.35rem; margin:.2rem 0 1rem; } h2 { font-size:1.1rem; margin:1.4rem 0 .6rem; }
.cards { display:grid; grid-template-columns:repeat(auto-fit, minmax(170px, 1fr)); gap:.8rem; }
.card { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:.8rem 1rem; }
.card .n { font-size:1.8rem; font-weight:600; }
.box { background:var(--panel); border:1px solid var(--line); border-radius:8px; padding:.8rem 1rem; overflow-x:auto; }
table { border-collapse:collapse; width:100%; }
th, td { text-align:left; padding:.4rem .6rem; border-bottom:1px solid var(--line); vertical-align:top; }
th { font-weight:600; color:var(--muted); font-size:.85rem; }
tr:last-child td { border-bottom:none; }
.muted { color:var(--muted); } .small { font-size:.85rem; }
.dot { display:inline-block; width:.65rem; height:.65rem; border-radius:50%; margin-right:.35rem; vertical-align:baseline; }
.ok { color:var(--ok); } .bad { color:var(--bad); } .warn { color:var(--warn); }
.dot.ok { background:var(--ok); } .dot.fehler { background:var(--bad); } .dot.nicht_erreichbar { background:var(--off); } .dot.unbekannt { background:var(--line); }
.tag { display:inline-block; padding:0 .4rem; border-radius:4px; font-size:.8rem; border:1px solid var(--line); }
.tag.old { border-color:var(--warn); color:var(--warn); }
form.inline { display:inline; }
input, select, textarea, button { font:inherit; color:inherit; background:var(--panel); border:1px solid var(--line); border-radius:6px; padding:.35rem .55rem; }
button, .button { background:var(--accent); color:#fff; border:none; cursor:pointer; padding:.4rem .8rem; border-radius:6px; display:inline-block; }
button.plain { background:none; color:var(--accent); padding:0; }
textarea { width:100%; }
textarea.log, pre.log { font:12px/1.4 ui-monospace, Menlo, Consolas, monospace; }
pre.log { white-space:pre-wrap; word-break:break-all; max-height:70vh; overflow:auto; margin:0; }
pre.log .E { color:var(--bad); } pre.log .W { color:var(--warn); }
.filters { display:flex; flex-wrap:wrap; gap:.5rem; align-items:center; margin-bottom:.8rem; }
.notice { padding:.6rem .9rem; border-left:4px solid var(--warn); background:var(--panel); margin:.6rem 0; }
.bars { display:flex; align-items:flex-end; gap:4px; height:90px; }
.bars div { flex:1; background:var(--accent); min-height:1px; border-radius:3px 3px 0 0; }
.barlabels { display:flex; gap:4px; } .barlabels span { flex:1; text-align:center; font-size:.75rem; color:var(--muted); }
footer { text-align:center; color:var(--muted); font-size:.8rem; padding:1.5rem; }
.login { max-width:360px; margin:12vh auto; }
.login label { display:block; margin:.6rem 0 .2rem; }
.login input { width:100%; }
"#;

fn page(user: Option<&User>, title: &str, body: &str) -> Html<String> {
    let nav = match user {
        Some(u) => format!(
            r#"<nav><a href="/">Übersicht</a><a href="/arbeitsplaetze">Arbeitsplätze</a><a href="/berichte">Berichte</a></nav>
<span class="muted small">{} ({})</span>
<form method="post" action="/logout"><button class="plain">Abmelden</button></form>"#,
            e(&u.name),
            e(&u.role)
        ),
        None => String::new(),
    };
    Html(format!(
        r#"<!doctype html><html lang="de"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{} · StarDash</title><style>{CSS}</style></head>
<body><header><b>StarDash</b>{nav}</header><main>{body}</main>
<footer>StarDash {VERSION} · zeigt den Zustand der Programme, nicht das Verhalten von Personen</footer></body></html>"#,
        e(title)
    ))
}

fn not_found(user: &User) -> Response {
    (
        StatusCode::NOT_FOUND,
        page(Some(user), "Nicht gefunden", "<h1>Nicht gefunden</h1>"),
    )
        .into_response()
}

fn failed(user: &User, err: impl std::fmt::Display) -> Response {
    tracing::error!(error = %err, "Weboberfläche");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        page(
            Some(user),
            "Fehler",
            &format!("<h1>Fehler</h1><p>{}</p>", e(&err.to_string())),
        ),
    )
        .into_response()
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

/// Angemeldeter Benutzer oder Umleitung zur Anmeldung
async fn user(app: &App, headers: &HeaderMap) -> Result<User, Response> {
    if let Some(token) = cookie_value(headers, COOKIE)
        && let Ok(Some(u)) = app.db.session_user(&auth::hash_token(token)).await
    {
        return Ok(u);
    }
    Err(Redirect::to("/login").into_response())
}

/// Schutz gegen Formulare fremder Seiten: Origin muss zum Host passen
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true; // ältere Browser; das Cookie ist ohnehin SameSite=Strict
    };
    let host = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    origin
        .split_once("://")
        .map(|(_, h)| h.trim_end_matches('/'))
        .is_some_and(|h| h.eq_ignore_ascii_case(host))
}

fn https(headers: &HeaderMap) -> bool {
    headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"))
}

fn allowed(user: &User, company: i64) -> bool {
    user.company == 0 || user.company == company
}

// ---------------------------------------------------------------------------
// Anmeldung

async fn login_form(State(app): State<Arc<App>>) -> Html<String> {
    let hint = match app.db.user_count().await {
        Ok(0) => {
            r#"<p class="notice">Noch kein Benutzer. Auf dem Server anlegen mit <code>sudo stardash benutzer NAME</code>.</p>"#
        }
        _ => "",
    };
    login_page(hint, "")
}

fn login_page(hint: &str, error: &str) -> Html<String> {
    let error = if error.is_empty() {
        String::new()
    } else {
        format!(r#"<p class="bad">{}</p>"#, e(error))
    };
    page(
        None,
        "Anmelden",
        &format!(
            r#"<div class="login box"><h1>Anmelden</h1>{hint}{error}
<form method="post" action="/login"><label>Benutzer<input name="name" autocomplete="username" required autofocus></label>
<label>Passwort<input name="password" type="password" autocomplete="current-password" required></label>
<p><button>Anmelden</button></p></form></div>"#
        ),
    )
}

#[derive(Deserialize)]
struct LoginForm {
    name: String,
    password: String,
}

async fn login(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Form(f): Form<LoginForm>,
) -> Response {
    if !same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let found = app.db.user_login(f.name.trim()).await.ok().flatten();
    let ok = match &found {
        Some((_, hash)) => auth::verify_password(&f.password, hash),
        None => {
            // gleich lange rechnen, ob es den Benutzer gibt oder nicht
            let _ = auth::verify_password(
                &f.password,
                "$argon2id$v=19$m=19456,t=2,p=1$c2FsdHNhbHRzYWx0$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
            false
        }
    };
    let Some((id, _)) = found.filter(|_| ok) else {
        tracing::warn!(name = f.name, "Anmeldung abgelehnt");
        return (
            StatusCode::UNAUTHORIZED,
            login_page("", "Benutzer oder Passwort falsch."),
        )
            .into_response();
    };
    let token = auth::random_token(32);
    if let Err(err) = app
        .db
        .add_session(&auth::hash_token(&token), id, now() + SESSION_SECS)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
    }
    let secure = if https(&headers) { "; Secure" } else { "" };
    let cookie = format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_SECS}{secure}"
    );
    let mut res = Redirect::to("/").into_response();
    if let Ok(v) = HeaderValue::from_str(&cookie) {
        res.headers_mut().insert(header::SET_COOKIE, v);
    }
    res
}

async fn logout(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_value(&headers, COOKIE) {
        let _ = app.db.remove_session(&auth::hash_token(token)).await;
    }
    let mut res = Redirect::to("/login").into_response();
    res.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static("stardash_session=; Path=/; Max-Age=0"),
    );
    res
}

// ---------------------------------------------------------------------------
// Übersicht

fn version_tag(app: &App, c_app: &str, version: &str) -> String {
    match app.latest(c_app) {
        Some(l) if releases::older(version, &l) => format!(
            r#"<span class="tag old" title="Neueste Version: {}">{} · veraltet</span>"#,
            e(&l),
            e(version)
        ),
        _ => format!(r#"<span class="tag">{}</span>"#, e(version)),
    }
}

async fn overview(State(app): State<Arc<App>>, headers: HeaderMap) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company = user.company_filter();
    let clients = match app.db.clients(company).await {
        Ok(c) => c,
        Err(err) => return failed(&user, err),
    };
    let errors = app.db.errors(company, 7).await.unwrap_or_default();
    let recent = app
        .db
        .reports(&ReportFilter {
            company,
            limit: 10,
            ..Default::default()
        })
        .await
        .unwrap_or_default();

    let mut health: BTreeMap<Health, usize> = BTreeMap::new();
    let mut versions: BTreeMap<(String, String), usize> = BTreeMap::new();
    for c in &clients {
        *health
            .entry(Health::of(c, app.config.offline_after_minutes))
            .or_default() += 1;
        *versions
            .entry((c.app.clone(), c.version.clone()))
            .or_default() += 1;
    }
    let mut b = String::from("<h1>Übersicht</h1><div class=\"cards\">");
    let _ = write!(
        b,
        r#"<div class="card"><div class="n">{}</div>Arbeitsplätze</div>"#,
        clients.len()
    );
    for h in [Health::Ok, Health::Fehler, Health::NichtErreichbar] {
        let _ = write!(
            b,
            r#"<a class="card" href="/arbeitsplaetze?zustand={}"><div class="n"><span class="dot {}"></span>{}</div>{}</a>"#,
            h.key(),
            h.key(),
            health.get(&h).copied().unwrap_or(0),
            h.label()
        );
    }
    b.push_str("</div>");

    // Versionen
    b.push_str("<h2>Versionen im Einsatz</h2><div class=\"box\"><table><tr><th>App</th><th>Version</th><th>Arbeitsplätze</th></tr>");
    for ((a, v), n) in versions.iter().rev() {
        let _ = write!(
            b,
            r#"<tr><td>{}</td><td>{}</td><td><a href="/arbeitsplaetze?app={}&version={}">{n}</a></td></tr>"#,
            e(a),
            version_tag(&app, a, v),
            urlencoding::encode(a),
            urlencoding::encode(v)
        );
    }
    if versions.is_empty() {
        b.push_str(r#"<tr><td colspan="3" class="muted">Noch keine Meldungen. Adresse und Schlüssel in die Vorgabedatei der Clients eintragen (<code>[Support]</code>).</td></tr>"#);
    }
    b.push_str("</table>");
    if let Ok(latest) = app.latest.read()
        && !latest.is_empty()
    {
        let list: Vec<String> = latest
            .iter()
            .map(|(a, v)| format!("{} {}", e(a), e(v)))
            .collect();
        let _ = write!(
            b,
            r#"<p class="small muted">Neueste Versionen: {}</p>"#,
            list.join(", ")
        );
    }
    b.push_str("</div>");

    // Fehler der letzten 7 Tage
    let today = now() / 86_400;
    let mut per_day = [0i64; 7];
    for (day, _, _, _, n) in &errors {
        let i = (day - (today - 6)) as usize;
        if i < 7 {
            per_day[i] += n;
        }
    }
    let max = per_day.iter().copied().max().unwrap_or(0).max(1);
    b.push_str("<h2>Fehler der letzten 7 Tage</h2><div class=\"box\"><div class=\"bars\">");
    for n in per_day {
        let _ = write!(
            b,
            r#"<div style="height:{}%" title="{n} Fehler"></div>"#,
            n * 100 / max
        );
    }
    b.push_str("</div><div class=\"barlabels\">");
    for i in 0..7 {
        let d = when((today - 6 + i) * 86_400);
        let _ = write!(b, "<span>{}</span>", &d[..6]);
    }
    b.push_str("</div>");
    let by_kind = crate::db::sum_by_kind(&errors);
    let mut by_version: BTreeMap<(String, String), i64> = BTreeMap::new();
    for (_, a, v, _, n) in &errors {
        *by_version.entry((a.clone(), v.clone())).or_default() += n;
    }
    if !by_kind.is_empty() {
        let kinds: Vec<String> = by_kind
            .iter()
            .map(|(k, n)| format!("{}: {n}", e(&kind_label(k))))
            .collect();
        let vers: Vec<String> = by_version
            .iter()
            .map(|((a, v), n)| format!("{} {}: {n}", e(a), e(v)))
            .collect();
        let _ = write!(
            b,
            r#"<p class="small">Nach Art: {}<br>Nach Version: {}</p>"#,
            kinds.join(" · "),
            vers.join(" · ")
        );
    }
    b.push_str("</div>");

    // Neueste Berichte
    b.push_str("<h2>Neueste Berichte</h2>");
    b.push_str(&report_table(&recent));
    page(Some(&user), "Übersicht", &b).into_response()
}

fn kind_label(k: &str) -> String {
    match k {
        "sip" => "Softphone".into(),
        "login" => "Anmeldung".into(),
        "connection" => "Verbindung".into(),
        "audio" => "Audio".into(),
        other => other.into(),
    }
}

fn report_table(rows: &[ReportRow]) -> String {
    let mut b = String::from(
        r#"<div class="box"><table><tr><th>Nr.</th><th>Erhalten</th><th>Anlass</th><th>Arbeitsplatz</th><th>Version</th><th>Text</th><th></th></tr>"#,
    );
    for r in rows {
        let text = if r.comment.is_empty() {
            &r.summary
        } else {
            &r.comment
        };
        let _ = write!(
            b,
            r#"<tr><td><a href="/berichte/{id}"><code>{id}</code></a></td><td class="small">{}</td><td>{}</td><td>{}</td><td>{} {}</td><td class="small">{}</td><td>{}</td></tr>"#,
            ago(r.received),
            e(reason_label(&r.reason)),
            e(&r.hostname),
            e(&r.app),
            e(&r.version),
            e(crate::db::cut(text, 120)),
            if r.done {
                "<span class=\"ok small\">erledigt</span>".to_string()
            } else if !r.issue_url.is_empty() {
                "<span class=\"small\">Issue</span>".to_string()
            } else {
                String::new()
            },
            id = e(&r.id),
        );
    }
    if rows.is_empty() {
        b.push_str(r#"<tr><td colspan="7" class="muted">Keine Berichte.</td></tr>"#);
    }
    b.push_str("</table></div>");
    b
}

// ---------------------------------------------------------------------------
// Arbeitsplätze

#[derive(Deserialize, Default)]
struct ClientQuery {
    #[serde(default)]
    app: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    zustand: String,
    #[serde(default)]
    q: String,
}

fn yes(ok: bool, label: &str) -> String {
    format!(
        r#"<span class="{}" title="{label}">{} {label}</span>"#,
        if ok { "ok" } else { "bad" },
        if ok { "●" } else { "○" }
    )
}

fn client_state(c: &ClientRow, h: Health) -> String {
    if !c.heartbeat || h == Health::NichtErreichbar {
        return format!(r#"<span class="muted">{}</span>"#, h.label());
    }
    let mut s = vec![
        yes(c.pbx_connected, "Anlage"),
        yes(c.chat_connected, "Chat"),
    ];
    if c.softphone_enabled {
        s.push(yes(c.softphone_registered, "Softphone"));
    }
    if !c.audio_ok {
        s.push(yes(false, "Audio"));
    }
    s.join(" ")
}

async fn clients(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<ClientQuery>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let all = match app.db.clients(user.company_filter()).await {
        Ok(c) => c,
        Err(err) => return failed(&user, err),
    };
    let off = app.config.offline_after_minutes;
    let list: Vec<&ClientRow> = all
        .iter()
        .filter(|c| q.app.is_empty() || c.app == q.app)
        .filter(|c| q.version.is_empty() || c.version == q.version)
        .filter(|c| q.zustand.is_empty() || Health::of(c, off).key() == q.zustand)
        .filter(|c| {
            q.q.is_empty()
                || c.hostname.to_lowercase().contains(&q.q.to_lowercase())
                || c.pbx.contains(&q.q)
        })
        .collect();
    let mut b = String::from("<h1>Arbeitsplätze</h1>");
    let _ = write!(
        b,
        r#"<form class="filters" method="get"><input name="q" placeholder="Rechnername" value="{}">
<select name="zustand"><option value="">Alle Zustände</option>{}</select>
<input name="app" placeholder="App" value="{}" size="8"><input name="version" placeholder="Version" value="{}" size="8">
<button>Filtern</button> <a href="/arbeitsplaetze">zurücksetzen</a></form>"#,
        e(&q.q),
        [
            Health::Ok,
            Health::Fehler,
            Health::NichtErreichbar,
            Health::Unbekannt
        ]
        .iter()
        .map(|h| format!(
            r#"<option value="{}"{}>{}</option>"#,
            h.key(),
            if q.zustand == h.key() {
                " selected"
            } else {
                ""
            },
            h.label()
        ))
        .collect::<String>(),
        e(&q.app),
        e(&q.version)
    );
    b.push_str(r#"<div class="box"><table><tr><th>Rechner</th><th>App</th><th>Version</th><th>System</th><th>Anlage</th><th>Zustand</th><th>Letzte Meldung</th><th>Berichte 24 h</th></tr>"#);
    for c in &list {
        let h = Health::of(c, off);
        let _ = write!(
            b,
            r#"<tr><td><span class="dot {}"></span><a href="/arbeitsplaetze/{}/{}">{}</a></td><td>{}</td><td>{}</td><td class="small">{}<br><span class="muted">{}</span></td><td class="small">{}</td><td class="small">{}</td><td class="small">{}</td><td>{}</td></tr>"#,
            h.key(),
            c.company,
            urlencoding::encode(&c.install_id),
            e(&c.hostname),
            e(&c.app),
            version_tag(&app, &c.app, &c.version),
            e(crate::db::cut(&c.os, 60)),
            e(&c.package),
            e(&c.pbx),
            client_state(c, h),
            ago(c.last_seen),
            if c.reports_24h > 0 {
                format!(
                    r#"<a href="/berichte?arbeitsplatz={}">{}</a>"#,
                    urlencoding::encode(&c.install_id),
                    c.reports_24h
                )
            } else {
                "0".into()
            }
        );
    }
    if list.is_empty() {
        b.push_str(r#"<tr><td colspan="8" class="muted">Keine Arbeitsplätze.</td></tr>"#);
    }
    b.push_str("</table></div>");
    let _ = write!(
        b,
        r#"<p class="small muted">{} von {} Arbeitsplätzen</p>"#,
        list.len(),
        all.len()
    );
    page(Some(&user), "Arbeitsplätze", &b).into_response()
}

async fn client(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path((company, install)): Path<(i64, String)>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !allowed(&user, company) {
        return not_found(&user);
    }
    let c = match app.db.client(company, &install).await {
        Ok(Some(c)) => c,
        Ok(None) => return not_found(&user),
        Err(err) => return failed(&user, err),
    };
    let h = Health::of(&c, app.config.offline_after_minutes);
    let reports = app
        .db
        .reports(&ReportFilter {
            company: Some(company),
            install_id: Some(install.clone()),
            limit: 100,
            ..Default::default()
        })
        .await
        .unwrap_or_default();
    let mut b = format!(
        r#"<h1><span class="dot {}"></span>{} · {}</h1><div class="box"><table>"#,
        h.key(),
        e(&c.hostname),
        e(&c.app)
    );
    let rows = [
        ("Zustand", format!("{} {}", h.label(), client_state(&c, h))),
        ("Version", version_tag(&app, &c.app, &c.version)),
        ("System", e(&c.os)),
        ("Paket", e(&c.package)),
        ("Anlage", format!("{} {}", e(&c.pbx), e(&c.pbx_version))),
        (
            "Softphone eingerichtet",
            if c.softphone_enabled {
                "ja".into()
            } else {
                "nein".into()
            },
        ),
        (
            "Letzte Meldung",
            format!("{} ({})", when(c.last_seen), ago(c.last_seen)),
        ),
        (
            "Läuft seit",
            if c.uptime_secs > 0 {
                format!("{} h", c.uptime_secs / 3600)
            } else {
                "–".into()
            },
        ),
        ("Letzter Fehler", when(c.last_error)),
        ("Zuerst gesehen", when(c.first_seen)),
        (
            "Installations-Kennung",
            format!("<code>{}</code>", e(&c.install_id)),
        ),
    ];
    for (k, v) in rows {
        let _ = write!(b, "<tr><th>{k}</th><td>{v}</td></tr>");
    }
    b.push_str("</table></div><h2>Berichte</h2>");
    b.push_str(&report_table(&reports));
    page(Some(&user), &c.hostname, &b).into_response()
}

// ---------------------------------------------------------------------------
// Berichte

#[derive(Deserialize, Default)]
struct ReportQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    anlass: String,
    #[serde(default)]
    offen: String,
    #[serde(default)]
    arbeitsplatz: String,
}

async fn reports(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Query(q): Query<ReportQuery>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    // Bericht-Nr. direkt öffnen
    let wanted = q.q.trim().to_uppercase();
    if valid_report_id(&wanted)
        && let Ok(Some(r)) = app.db.report(&wanted).await
        && allowed(&user, r.company)
    {
        return Redirect::to(&format!("/berichte/{wanted}")).into_response();
    }
    let rows = match app
        .db
        .reports(&ReportFilter {
            company: user.company_filter(),
            install_id: (!q.arbeitsplatz.is_empty()).then(|| q.arbeitsplatz.clone()),
            reason: (!q.anlass.is_empty()).then(|| q.anlass.clone()),
            search: (!q.q.trim().is_empty()).then(|| q.q.clone()),
            open_only: q.offen == "1",
            limit: 300,
        })
        .await
    {
        Ok(r) => r,
        Err(err) => return failed(&user, err),
    };
    let mut b = String::from("<h1>Berichte</h1>");
    let _ = write!(
        b,
        r#"<form class="filters" method="get"><input name="q" placeholder="Bericht-Nr., Rechner oder Version" value="{}" size="28">
<select name="anlass"><option value="">Alle Anlässe</option>{}</select>
<label><input type="checkbox" name="offen" value="1"{}> nur offene</label>
<button>Suchen</button> <a href="/berichte">zurücksetzen</a></form>"#,
        e(&q.q),
        REASONS
            .iter()
            .map(|(k, l)| format!(
                r#"<option value="{k}"{}>{l}</option>"#,
                if q.anlass == *k { " selected" } else { "" }
            ))
            .collect::<String>(),
        if q.offen == "1" { " checked" } else { "" }
    );
    b.push_str(&report_table(&rows));
    page(Some(&user), "Berichte", &b).into_response()
}

async fn load_report(app: &App, user: &User, id: &str) -> Result<ReportRow, Response> {
    let id = id.to_uppercase();
    if !valid_report_id(&id) {
        return Err(not_found(user));
    }
    match app.db.report(&id).await {
        Ok(Some(r)) if allowed(user, r.company) => Ok(r),
        Ok(_) => Err(not_found(user)),
        Err(err) => Err(failed(user, err)),
    }
}

/// Protokoll eines Berichts als Text
async fn log_text(app: &App, r: &ReportRow) -> String {
    let Some(gz) = app.report_log(r.company, &r.id).await else {
        return String::new();
    };
    let mut out = String::new();
    let _ = flate2::read::GzDecoder::new(&gz[..])
        .take(128 * 1024 * 1024)
        .read_to_string(&mut out);
    out
}

fn highlight(log: &str) -> String {
    let mut out = String::with_capacity(log.len() + log.len() / 8);
    for line in log.lines() {
        let class = if line.contains(" ERROR ") {
            Some("E")
        } else if line.contains(" WARN ") {
            Some("W")
        } else {
            None
        };
        match class {
            Some(c) => {
                let _ = writeln!(out, r#"<span class="{c}">{}</span>"#, e(line));
            }
            None => {
                out.push_str(&e(line));
                out.push('\n');
            }
        }
    }
    out
}

async fn report(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let r = match load_report(&app, &user, &id).await {
        Ok(r) => r,
        Err(res) => return res,
    };
    let log = log_text(&app, &r).await;
    let problems = issues::problem_lines(&log, 30);
    let mut b = format!(
        r#"<h1>Bericht <code>{}</code> · {}</h1><div class="box"><table>"#,
        e(&r.id),
        e(reason_label(&r.reason))
    );
    let rows = [
        (
            "Arbeitsplatz",
            format!(
                r#"<a href="/arbeitsplaetze/{}/{}">{}</a>"#,
                r.company,
                urlencoding::encode(&r.install_id),
                e(&r.hostname)
            ),
        ),
        (
            "App",
            format!("{} {}", e(&r.app), version_tag(&app, &r.app, &r.version)),
        ),
        ("System", format!("{} · {}", e(&r.os), e(&r.package))),
        ("Anlage", format!("{} {}", e(&r.pbx), e(&r.pbx_version))),
        (
            "Zeit",
            format!("{} (erhalten {})", when(r.time as i64), ago(r.received)),
        ),
        ("Meldung", e(&r.summary)),
        (
            "Text des Benutzers",
            if r.comment.is_empty() {
                "–".into()
            } else {
                format!("<strong>{}</strong>", e(&r.comment))
            },
        ),
        (
            "Protokoll",
            format!(
                r#"{} · <a href="/berichte/{}/protokoll">herunterladen</a>"#,
                size(r.log_size),
                e(&r.id)
            ),
        ),
        (
            "Issue",
            if r.issue_url.is_empty() {
                "–".into()
            } else {
                format!(
                    r#"<a href="{0}" rel="noreferrer" target="_blank">{0}</a>"#,
                    e(&r.issue_url)
                )
            },
        ),
    ];
    for (k, v) in rows {
        let _ = write!(b, "<tr><th>{k}</th><td>{v}</td></tr>");
    }
    b.push_str("</table></div>");
    if user.may_edit() {
        let _ = write!(
            b,
            r#"<h2>Bearbeitung</h2><div class="box"><form method="post" action="/berichte/{id}/status">
<p><label><input type="checkbox" name="done" value="1"{}> erledigt</label></p>
<p><textarea name="note" rows="2" placeholder="Notiz für den Support">{}</textarea></p>
<p><button>Speichern</button>{}</p></form></div>"#,
            if r.done { " checked" } else { "" },
            e(&r.note),
            if issues::targets(&app.config, &r.app).is_empty() {
                String::new()
            } else {
                format!(
                    r#" &nbsp; <a class="button" href="/berichte/{}/issue">Issue erstellen</a>"#,
                    e(&r.id)
                )
            },
            id = e(&r.id)
        );
    } else if !r.note.is_empty() {
        let _ = write!(b, r#"<h2>Notiz</h2><div class="box">{}</div>"#, e(&r.note));
    }
    if !problems.is_empty() {
        b.push_str(r#"<h2>Fehler und Warnungen</h2><div class="box"><pre class="log">"#);
        b.push_str(&highlight(&problems.join("\n")));
        b.push_str("</pre></div>");
    }
    b.push_str(r#"<h2>Protokoll</h2><div class="box"><pre class="log">"#);
    b.push_str(&highlight(issues::excerpt(&log)));
    b.push_str("</pre>");
    if log.len() > issues::EXCERPT_MAX {
        b.push_str(r#"<p class="small muted">Gezeigt wird der neueste Teil; das ganze Protokoll gibt es zum Herunterladen.</p>"#);
    }
    b.push_str("</div>");
    page(Some(&user), &r.id, &b).into_response()
}

async fn report_log(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let r = match load_report(&app, &user, &id).await {
        Ok(r) => r,
        Err(res) => return res,
    };
    let log = log_text(&app, &r).await;
    (
        [
            (
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"stardash-{}.log\"", r.id),
            ),
        ],
        log,
    )
        .into_response()
}

#[derive(Deserialize)]
struct StateForm {
    #[serde(default)]
    done: String,
    #[serde(default)]
    note: String,
}

async fn report_state(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(f): Form<StateForm>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !same_origin(&headers) || !user.may_edit() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let r = match load_report(&app, &user, &id).await {
        Ok(r) => r,
        Err(res) => return res,
    };
    if let Err(err) = app.db.set_report_state(&r.id, f.done == "1", &f.note).await {
        return failed(&user, err);
    }
    Redirect::to(&format!("/berichte/{}", r.id)).into_response()
}

// ---------------------------------------------------------------------------
// Issue erstellen

#[derive(Deserialize, Default)]
struct IssueQuery {
    #[serde(default)]
    ziel: Option<usize>,
}

async fn issue_form(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<IssueQuery>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !user.may_edit() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let r = match load_report(&app, &user, &id).await {
        Ok(r) => r,
        Err(res) => return res,
    };
    let targets = issues::targets(&app.config, &r.app);
    if targets.is_empty() {
        return not_found(&user);
    }
    let chosen = q
        .ziel
        .and_then(|i| targets.iter().find(|t| t.id == i))
        .unwrap_or(&targets[0]);
    let log = log_text(&app, &r).await;
    let (title, mut body) = issues::draft(&r);
    let problems = issues::problem_lines(&log, 15);
    if !problems.is_empty() {
        body.push_str("\n**Fehler und Warnungen:**\n\n```text\n");
        body.push_str(&problems.join("\n").replace("```", "'''"));
        body.push_str("\n```\n");
    }
    let mut b = format!(
        r#"<h1>Issue erstellen · Bericht <code>{}</code></h1>"#,
        e(&r.id)
    );
    if targets.len() > 1 {
        b.push_str(r#"<form class="filters" method="get"><label>Ziel <select name="ziel" onchange="this.form.submit()">"#);
        for t in &targets {
            let _ = write!(
                b,
                r#"<option value="{}"{}>{}</option>"#,
                t.id,
                if t.id == chosen.id { " selected" } else { "" },
                e(&t.label())
            );
        }
        b.push_str("</select></label> <noscript><button>Wählen</button></noscript></form>");
    } else {
        let _ = write!(b, "<p>Ziel: <strong>{}</strong></p>", e(&chosen.label()));
    }
    if chosen.public {
        b.push_str(r#"<p class="notice"><strong>Öffentlich:</strong> Issues in diesem Repository kann jeder im Internet lesen. Prüfe Titel, Text und Protokoll unten, bevor du absendest. Interne Nummern stehen standardmässig im Klartext; mit dem Häkchen werden sie unkenntlich.</p>"#);
    }
    if chosen.kind == crate::config::TargetKind::Github && chosen.token.is_empty() {
        b.push_str(r#"<p class="notice">Für dieses Ziel ist kein Token eingetragen: StarDash öffnet das vorausgefüllte Issue in deinem Browser, wo du es mit deinem GitHub-Konto absendest. Das Protokoll lädst du vorher herunter und hängst es dort an.</p>"#);
    }
    let _ = write!(
        b,
        r#"<form method="post" action="/berichte/{id}/issue"><input type="hidden" name="ziel" value="{}">
<div class="box"><p><label>Titel<br><input name="title" value="{}" style="width:100%" required></label></p>
<p><label>Text<br><textarea name="body" rows="14">{}</textarea></label></p>
<p><label>Protokoll (wird angehängt; du kannst Zeilen entfernen)<br><textarea class="log" name="log" rows="18">{}</textarea></label></p>
<p><label><input type="checkbox" name="hide_internal" value="1"{}> Interne Nummern unkenntlich machen</label></p>
<p><button>Issue erstellen</button> <a href="/berichte/{id}">Abbrechen</a></p></div></form>"#,
        chosen.id,
        e(&title),
        e(&body),
        e(issues::excerpt(&log)),
        if chosen.public { " checked" } else { "" },
        id = e(&r.id),
    );
    page(Some(&user), "Issue erstellen", &b).into_response()
}

#[derive(Deserialize)]
struct IssueForm {
    ziel: usize,
    title: String,
    body: String,
    #[serde(default)]
    log: String,
    #[serde(default)]
    hide_internal: String,
}

async fn issue_create(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(f): Form<IssueForm>,
) -> Response {
    let user = match user(&app, &headers).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if !same_origin(&headers) || !user.may_edit() {
        return StatusCode::FORBIDDEN.into_response();
    }
    let r = match load_report(&app, &user, &id).await {
        Ok(r) => r,
        Err(res) => return res,
    };
    let targets = issues::targets(&app.config, &r.app);
    let Some(target) = targets.iter().find(|t| t.id == f.ziel) else {
        return not_found(&user);
    };
    let (title, body, log) = if f.hide_internal == "1" {
        (
            issues::hide_internal(&f.title),
            issues::hide_internal(&f.body),
            issues::hide_internal(&f.log),
        )
    } else {
        (f.title, f.body, f.log)
    };
    match issues::create(&app.http, target, &r.id, &title, &body, &log).await {
        Ok(Created::Issue(url)) => {
            let _ = app.db.set_issue_url(&r.id, &url).await;
            tracing::info!(id = r.id, url, user = user.name, "Issue erstellt");
            Redirect::to(&format!("/berichte/{}", r.id)).into_response()
        }
        Ok(Created::Browser(url)) => {
            let b = format!(
                r#"<h1>Issue im Browser absenden</h1><div class="box"><ol>
<li><a href="/berichte/{id}/protokoll">Protokoll herunterladen</a></li>
<li><a class="button" href="{url}" target="_blank" rel="noreferrer">Vorausgefülltes Issue öffnen</a></li>
<li>Dort die Protokolldatei in das Textfeld ziehen und absenden.</li>
<li>Den Link des Issues hier eintragen:</li></ol>
<form method="post" action="/berichte/{id}/status">{done}
<p class="small muted">(Optional) Link zum Issue als Notiz speichern:</p>
<p><input name="note" value="{note}" style="width:100%" placeholder="https://github.com/…/issues/…"></p><p><button>Speichern</button></p></form></div>"#,
                id = e(&r.id),
                url = e(&url),
                note = e(&r.note),
                done = if r.done {
                    r#"<input type="hidden" name="done" value="1">"#
                } else {
                    ""
                },
            );
            page(Some(&user), "Issue im Browser", &b).into_response()
        }
        Err(err) => {
            tracing::warn!(error = %err, "Issue nicht erstellt");
            page(
                Some(&user),
                "Issue nicht erstellt",
                &format!(
                    r#"<h1>Issue nicht erstellt</h1><p class="bad">{}</p><p><a href="/berichte/{}/issue">Zurück</a></p>"#,
                    e(&err.to_string()),
                    e(&r.id)
                ),
            )
            .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(when(0), "–");
        assert_eq!(when(1_795_300_000), "21.11.2026 22:26 UTC");
        assert_eq!(
            e("<a href=\"x\">&'"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn origin_check() {
        let mut h = HeaderMap::new();
        assert!(same_origin(&h));
        h.insert(header::HOST, HeaderValue::from_static("dash.firma.ch"));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://dash.firma.ch"),
        );
        assert!(same_origin(&h));
        h.insert(
            header::ORIGIN,
            HeaderValue::from_static("https://evil.example"),
        );
        assert!(!same_origin(&h));
    }
}
