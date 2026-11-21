//! Schlüssel, Passwörter und Anmeldung an der Weboberfläche.

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use sha2::{Digest, Sha256};

/// Zufälliger Schlüssel aus `bytes` Zufallsbytes, als Hex
pub fn random_token(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("Zufallszahlen vom System");
    hex(&buf)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Gespeichert wird nur der Hash eines Schlüssels
pub fn hash_token(token: &str) -> String {
    hex(&Sha256::digest(token.trim().as_bytes()))
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).expect("Zufallszahlen vom System");
    let salt = SaltString::encode_b64(&salt).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Bearer-Token aus dem Authorization-Kopf
pub fn bearer(headers: &axum::http::HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Vergleich ohne Abbruch beim ersten Unterschied
pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passwords_and_tokens() {
        let h = hash_password("geheim").unwrap();
        assert!(verify_password("geheim", &h));
        assert!(!verify_password("falsch", &h));
        let t = random_token(24);
        assert_eq!(t.len(), 48);
        assert_eq!(hash_token(&t), hash_token(&format!(" {t}\n")));
        assert!(same("abc", "abc") && !same("abc", "abd"));
    }
}
