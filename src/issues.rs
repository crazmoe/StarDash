//! „Issue erstellen“: Bericht als Issue mit Protokoll an GitHub, GitLab oder
//! Gitea/Forgejo.
//!
//! Vorgabe ab Werk ist ein Issue auf github.com im Repository der App
//! (crazmoe/StarCLX, crazmoe/StarCMX, crazmoe/StarMAC). Der Admin kann das in
//! der config.toml überschreiben (`modus = "eigene"`) oder abschalten
//! (`modus = "aus"`). Ohne Token für GitHub öffnet StarDash ein
//! vorausgefülltes Issue im Browser des Supports; das Protokoll lädt der
//! Support dann selbst herunter und hängt es an.

use std::sync::OnceLock;

use regex::Regex;

use crate::config::{Config, IssueMode, IssueTarget, TargetKind};
use crate::db::ReportRow;
use crate::model::reason_label;
use crate::releases::REPOS;

/// Höchstens so viel Protokoll im Formular (das Neueste)
pub const EXCERPT_MAX: usize = 200 * 1024;
/// Länge des Issue-Texts im Browser-Link (URLs werden sonst zu lang)
const BROWSER_BODY_MAX: usize = 6000;

#[derive(Debug, Clone)]
pub struct Target {
    pub id: usize,
    pub name: String,
    pub kind: TargetKind,
    pub url: String,
    pub project: String,
    pub token: String,
    /// Öffentlich sichtbar (github.com-Vorgabe)
    pub public: bool,
}

impl Target {
    pub fn label(&self) -> String {
        let kind = match self.kind {
            TargetKind::Github => "GitHub",
            TargetKind::Gitlab => "GitLab",
            TargetKind::Gitea => "Gitea",
        };
        if self.name.is_empty() {
            format!("{kind}: {}", self.project)
        } else {
            format!("{} ({kind}: {})", self.name, self.project)
        }
    }
}

/// Ziele für einen Bericht dieser App
pub fn targets(config: &Config, app: &str) -> Vec<Target> {
    let mut out = Vec::new();
    if config.issues.mode == IssueMode::Aus {
        return out;
    }
    if config.issues.mode == IssueMode::Vorgabe {
        let repo = REPOS
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(app))
            .map(|(_, r)| *r)
            .unwrap_or("crazmoe/StarCLX");
        out.push(Target {
            id: 0,
            name: "Entwickler von StarCLX/StarCMX/StarMAC".into(),
            kind: TargetKind::Github,
            url: "https://github.com".into(),
            project: repo.into(),
            token: config.issues.github_token.clone(),
            public: true,
        });
    }
    for (i, t) in config.issues.targets.iter().enumerate() {
        if !t.apps.is_empty() && !t.apps.iter().any(|a| a.eq_ignore_ascii_case(app)) {
            continue;
        }
        out.push(own(i + 1, t));
    }
    out
}

fn own(id: usize, t: &IssueTarget) -> Target {
    let url = if t.url.is_empty() && t.kind == TargetKind::Github {
        "https://github.com".to_string()
    } else {
        t.url.trim_end_matches('/').to_string()
    };
    Target {
        id,
        name: t.name.clone(),
        kind: t.kind,
        public: false,
        url,
        project: t.project.clone(),
        token: t.token.clone(),
    }
}

/// Vorschlag für Titel und Text
pub fn draft(r: &ReportRow) -> (String, String) {
    let what = if r.summary.is_empty() || r.reason == "manual" {
        reason_label(&r.reason).to_string()
    } else {
        r.summary.clone()
    };
    let title = format!("{} {}: {}", r.app, r.version, crate::db::cut(&what, 120));
    let mut body = String::new();
    if !r.comment.is_empty() {
        body.push_str("**Was ist passiert (Benutzer):**\n\n");
        for line in r.comment.lines() {
            body.push_str("> ");
            body.push_str(line);
            body.push('\n');
        }
        body.push('\n');
    }
    body.push_str(&format!(
        "| | |\n|---|---|\n| Anlass | {} |\n| {} | {} |\n| Betriebssystem | {} |\n| Paket | {} |\n| STARFACE | {} |\n| Bericht | {} (StarDash) |\n",
        reason_label(&r.reason),
        r.app,
        r.version,
        r.os,
        r.package,
        if r.pbx_version.is_empty() { "?" } else { &r.pbx_version },
        r.id,
    ));
    if !r.summary.is_empty() && r.reason != "manual" {
        body.push_str(&format!(
            "\n**Meldung:** `{}`\n",
            r.summary.replace('`', "'")
        ));
    }
    (title, body)
}

/// Fehler- und Warnzeilen aus dem Protokoll (für den Issue-Text)
pub fn problem_lines(log: &str, max: usize) -> Vec<&str> {
    let mut lines: Vec<&str> = log
        .lines()
        .filter(|l| l.contains(" ERROR ") || l.contains(" WARN "))
        .collect();
    if lines.len() > max {
        lines.drain(..lines.len() - max);
    }
    lines
}

/// Neuester Teil des Protokolls, ab einem Zeilenanfang
pub fn excerpt(log: &str) -> &str {
    if log.len() <= EXCERPT_MAX {
        return log;
    }
    let mut start = log.len() - EXCERPT_MAX;
    while !log.is_char_boundary(start) {
        start += 1;
    }
    match log[start..].find('\n') {
        Some(i) => &log[start + i + 1..],
        None => &log[start..],
    }
}

/// Macht interne Nummern (2–6 Stellen) in SIP-Adressen und Nummernfeldern
/// unkenntlich (für öffentliche Issues).
pub fn hide_internal(text: &str) -> String {
    static SIP: OnceLock<Regex> = OnceLock::new();
    static FIELD: OnceLock<Regex> = OnceLock::new();
    let sip = SIP.get_or_init(|| Regex::new(r"(sips?:)([0-9]{2,6})@").expect("Muster"));
    let field = FIELD.get_or_init(|| {
        Regex::new(r#"\b((?:[a-z]+_)*(?:number|nummer|phone|extension|caller|callee|from|to|target|destination|aor))(\s*[:=]\s*"?)([0-9]{2,6})\b"#)
            .expect("Muster")
    });
    let fp = |n: &str| {
        use sha2::{Digest, Sha256};
        let h = Sha256::digest(format!("stardash-intern:{n}").as_bytes());
        format!("intern#{:02x}{:02x}", h[0], h[1])
    };
    let s = sip.replace_all(text, |c: &regex::Captures| {
        format!("{}{}@", &c[1], fp(&c[2]))
    });
    field
        .replace_all(&s, |c: &regex::Captures| {
            format!("{}{}{}", &c[1], &c[2], fp(&c[3]))
        })
        .into_owned()
}

#[derive(Debug)]
pub enum Created {
    /// Issue angelegt
    Issue(String),
    /// Ohne Token: im Browser öffnen und dort absenden
    Browser(String),
}

/// Legt das Issue an. `log` ist das (vom Support geprüfte) Protokoll.
pub async fn create(
    http: &reqwest::Client,
    target: &Target,
    report_id: &str,
    title: &str,
    body: &str,
    log: &str,
) -> anyhow::Result<Created> {
    let file = format!("stardash-{report_id}.log");
    match target.kind {
        TargetKind::Github => github(http, target, &file, title, body, log).await,
        TargetKind::Gitlab => gitlab(http, target, &file, title, body, log).await,
        TargetKind::Gitea => gitea(http, target, &file, title, body, log).await,
    }
}

fn api_base_github(target: &Target) -> String {
    if target.url.is_empty() || target.url.contains("://github.com") {
        "https://api.github.com".into()
    } else {
        // GitHub Enterprise Server
        format!("{}/api/v3", target.url)
    }
}

async fn check(res: reqwest::Response) -> anyhow::Result<serde_json::Value> {
    let status = res.status();
    let text = res.text().await.unwrap_or_default();
    if !status.is_success() {
        anyhow::bail!("HTTP {status}: {}", crate::db::cut(&text, 300));
    }
    Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
}

async fn github(
    http: &reqwest::Client,
    t: &Target,
    file: &str,
    title: &str,
    body: &str,
    log: &str,
) -> anyhow::Result<Created> {
    let web = if t.url.is_empty() {
        "https://github.com"
    } else {
        &t.url
    };
    if t.token.is_empty() {
        let mut text = body.to_string();
        text.push_str(&format!(
            "\n**Protokoll:** bitte die Datei `{file}` aus StarDash herunterladen und hier anhängen.\n"
        ));
        let text = crate::db::cut(&text, BROWSER_BODY_MAX);
        let url = format!(
            "{web}/{}/issues/new?title={}&body={}",
            t.project,
            urlencoding::encode(title),
            urlencoding::encode(text)
        );
        return Ok(Created::Browser(url));
    }
    let api = api_base_github(t);
    // Protokoll als geheimer Gist (nur per Link sichtbar)
    let gist = check(
        http.post(format!("{api}/gists"))
            .bearer_auth(&t.token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({
                "description": title,
                "public": false,
                "files": { file: { "content": if log.is_empty() { "(leer)" } else { log } } }
            }))
            .send()
            .await?,
    )
    .await?;
    let gist_url = gist["html_url"].as_str().unwrap_or_default();
    let text = format!("{body}\n**Protokoll:** {gist_url}\n");
    let issue = check(
        http.post(format!("{api}/repos/{}/issues", t.project))
            .bearer_auth(&t.token)
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({ "title": title, "body": text }))
            .send()
            .await?,
    )
    .await?;
    Ok(Created::Issue(
        issue["html_url"].as_str().unwrap_or_default().to_string(),
    ))
}

async fn gitlab(
    http: &reqwest::Client,
    t: &Target,
    file: &str,
    title: &str,
    body: &str,
    log: &str,
) -> anyhow::Result<Created> {
    let project = urlencoding::encode(&t.project).into_owned();
    let api = format!("{}/api/v4/projects/{project}", t.url);
    let part = reqwest::multipart::Part::bytes(log.as_bytes().to_vec())
        .file_name(file.to_string())
        .mime_str("text/plain")?;
    let upload = check(
        http.post(format!("{api}/uploads"))
            .header("PRIVATE-TOKEN", &t.token)
            .multipart(reqwest::multipart::Form::new().part("file", part))
            .send()
            .await?,
    )
    .await?;
    let markdown = upload["markdown"].as_str().unwrap_or_default();
    let issue = check(
        http.post(format!("{api}/issues"))
            .header("PRIVATE-TOKEN", &t.token)
            .json(&serde_json::json!({
                "title": title,
                "description": format!("{body}\n**Protokoll:** {markdown}\n"),
            }))
            .send()
            .await?,
    )
    .await?;
    Ok(Created::Issue(
        issue["web_url"].as_str().unwrap_or_default().to_string(),
    ))
}

async fn gitea(
    http: &reqwest::Client,
    t: &Target,
    file: &str,
    title: &str,
    body: &str,
    log: &str,
) -> anyhow::Result<Created> {
    let api = format!("{}/api/v1/repos/{}", t.url, t.project);
    let issue = check(
        http.post(format!("{api}/issues"))
            .header("Authorization", format!("token {}", t.token))
            .json(&serde_json::json!({ "title": title, "body": body }))
            .send()
            .await?,
    )
    .await?;
    let number = issue["number"].as_i64().unwrap_or_default();
    let part = reqwest::multipart::Part::bytes(log.as_bytes().to_vec())
        .file_name(file.to_string())
        .mime_str("text/plain")?;
    if let Err(e) = check(
        http.post(format!("{api}/issues/{number}/assets"))
            .header("Authorization", format!("token {}", t.token))
            .multipart(reqwest::multipart::Form::new().part("attachment", part))
            .send()
            .await?,
    )
    .await
    {
        tracing::warn!(error = %e, "Protokoll nicht an das Issue gehängt");
    }
    Ok(Created::Issue(
        issue["html_url"].as_str().unwrap_or_default().to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_targets_follow_app() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(targets(&c, "StarCMX")[0].project, "crazmoe/StarCMX");
        let c: Config = toml::from_str("[issues]\nmodus = \"aus\"").unwrap();
        assert!(targets(&c, "StarCLX").is_empty());
        let c: Config = toml::from_str(
            "[issues]\nmodus = \"eigene\"\n[[issues.ziel]]\nart = \"gitea\"\nurl = \"https://git.firma.ch/\"\nprojekt = \"it/tel\"\napps = [\"StarCMX\"]",
        )
        .unwrap();
        assert!(targets(&c, "StarCLX").is_empty());
        let t = &targets(&c, "StarCMX")[0];
        assert_eq!(t.url, "https://git.firma.ch");
        assert!(!t.public);
    }

    #[test]
    fn hides_internal_numbers() {
        let s = hide_internal("aor: \"sip:11@pbx\" target=12 port=5061 Versuch 3");
        assert!(!s.contains("sip:11@") && !s.contains("target=12"), "{s}");
        assert!(s.contains("port=5061") && s.contains("Versuch 3"));
    }

    #[tokio::test]
    async fn browser_link_without_token() {
        let c: Config = toml::from_str("").unwrap();
        let t = &targets(&c, "StarCLX")[0];
        let r = create(
            &reqwest::Client::new(),
            t,
            "K7-2F9A",
            "Titel ä",
            "Text",
            "log",
        )
        .await
        .unwrap();
        let Created::Browser(url) = r else { panic!() };
        assert!(
            url.starts_with("https://github.com/crazmoe/StarCLX/issues/new?title=Titel%20%C3%A4")
        );
    }
}
