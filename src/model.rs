//! Daten, die die Clients schicken (Schnittstelle v1, siehe `sf_support` in
//! StarCLX/StarCMX). Unbekannte Felder werden ignoriert, damit neuere Clients
//! mit älteren StarDash-Versionen funktionieren.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Client {
    pub install_id: String,
    pub hostname: String,
    pub app: String,
    pub version: String,
    pub os: String,
    pub package: String,
    pub pbx: String,
    pub pbx_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Report {
    pub id: String,
    pub reason: String,
    #[serde(default)]
    pub time: u64,
    pub client: Client,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub log: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Heartbeat {
    pub client: Option<Client>,
    pub time: u64,
    pub event: String,
    pub pbx_connected: bool,
    pub chat_connected: bool,
    pub softphone_registered: bool,
    pub softphone_enabled: bool,
    pub audio_ok: bool,
    pub uptime_secs: u64,
    pub last_error: u64,
    pub errors: BTreeMap<String, u32>,
}

/// Bericht-Nr. wie vom Client vergeben: `XX-XXXX` aus dem Alphabet ohne 0/O/1/I
pub fn valid_report_id(id: &str) -> bool {
    const ALPHABET: &str = "23456789ABCDEFGHJKLMNPQRSTUVWXYZ";
    id.len() == 7
        && id.as_bytes()[2] == b'-'
        && id
            .chars()
            .enumerate()
            .all(|(i, c)| i == 2 || ALPHABET.contains(c))
}

pub const REASONS: &[(&str, &str)] = &[
    ("manual", "Vom Benutzer gesendet"),
    ("crash", "Absturz"),
    ("sip", "Softphone"),
    ("login", "Anmeldung"),
    ("wake", "Nach Standby"),
];

pub fn reason_label(r: &str) -> &str {
    REASONS
        .iter()
        .find(|(k, _)| *k == r)
        .map(|(_, l)| crate::i18n::t(l))
        .unwrap_or(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_ids() {
        assert!(valid_report_id("K7-2F9A"));
        assert!(!valid_report_id("K7-2F9"));
        assert!(!valid_report_id("K0-2F9A"));
        assert!(!valid_report_id("../etc"));
    }

    #[test]
    fn accepts_client_json() {
        // So schickt sf_support einen Bericht
        let json = r#"{"id":"K7-2F9A","reason":"manual","time":1,"client":{"install_id":"i","hostname":"PC1","app":"StarCLX","version":"2.0.0","os":"linux","package":"deb","pbx":"pbx","pbx_version":"10","neu":1},"comment":"","summary":"","log":"x\n"}"#;
        let r: Report = serde_json::from_str(json).unwrap();
        assert_eq!(r.client.hostname, "PC1");
        let hb: Heartbeat =
            serde_json::from_str(r#"{"client":null,"time":1,"event":"start","errors":{"sip":2}}"#)
                .unwrap();
        assert_eq!(hb.errors["sip"], 2);
    }
}
