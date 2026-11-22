//! Neueste veröffentlichte Version der Clients, damit die Übersicht veraltete
//! Arbeitsplätze markieren kann. Abschaltbar mit `check_releases = false`.

use crate::App;

/// App → Repository auf GitHub
pub const REPOS: &[(&str, &str)] = &[
    ("StarCLX", "crazmoe/StarCLX"),
    // Ältere Windows-Clients heissen StarCMX und werden weiter mit dem
    // StarCMX-Repository verglichen; StarCLX unter Windows mit StarCLX.
    ("StarCMX", "crazmoe/StarCMX"),
    ("StarMAC", "crazmoe/StarMAC"),
];

pub async fn refresh(app: &App) {
    // Letzten Stand aus der Datenbank, auch ohne Internet
    if let Ok(Some(saved)) = app.db.setting("latest_versions").await
        && let Ok(map) = serde_json::from_str(&saved)
        && let Ok(mut latest) = app.latest.write()
    {
        *latest = map;
    }
    if !app.config.check_releases {
        return;
    }
    let mut changed = false;
    for (name, repo) in REPOS {
        let url = format!("https://api.github.com/repos/{repo}/releases/latest");
        let res = app
            .http
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .send()
            .await;
        let tag = match res {
            Ok(r) if r.status().is_success() => r
                .json::<serde_json::Value>()
                .await
                .ok()
                .and_then(|v| v["tag_name"].as_str().map(str::to_owned)),
            Ok(_) => None, // noch kein Release
            Err(e) => {
                tracing::debug!(error = %e, repo, "Neueste Version nicht abgefragt");
                return;
            }
        };
        if let Some(tag) = tag
            && let Ok(mut latest) = app.latest.write()
        {
            let v = tag.trim_start_matches('v').to_string();
            if latest.get(*name) != Some(&v) {
                latest.insert((*name).to_string(), v);
                changed = true;
            }
        }
    }
    if changed {
        let json = app
            .latest
            .read()
            .map(|l| serde_json::to_string(&*l).unwrap_or_default())
            .unwrap_or_default();
        let _ = app.db.set_setting("latest_versions", &json).await;
    }
}

/// Vergleicht Versionen wie "1.15.16" oder "2.0.0+123" (Zwischenbuild)
pub fn older(version: &str, latest: &str) -> bool {
    fn parts(v: &str) -> Vec<u64> {
        v.split(['+', '-'])
            .next()
            .unwrap_or("")
            .split('.')
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    }
    let (a, b) = (parts(version), parts(latest));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x < y;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::older;

    #[test]
    fn compares_versions() {
        assert!(older("1.15.16", "2.0.0"));
        assert!(older("2.0.0", "2.0.1"));
        assert!(!older("2.0.1", "2.0.1"));
        assert!(!older("2.0.1+57", "2.0.1"));
        assert!(!older("2.1.0", "2.0.9"));
    }
}
