//! Chiffrement au repos des secrets de configuration (REQ-SEC-004).
//!
//! AES-256-GCM (chiffrement authentifié) sur les valeurs **secrètes** des configurations de
//! canaux (mot de passe SMTP, jetons, clé utilisateur) : une fuite de sauvegarde de base ne
//! livre pas les identifiants. La clé est dérivée de la variable d'environnement
//! `ENCRYPTION_KEY` par SHA-256 (chaîne libre côté opérateur, clé de 32 octets côté crypto —
//! recommander 32+ octets aléatoires, ADR 0053). Format stocké :
//! `enc:v1:<base64(nonce)>:<base64(ciphertext+tag)>` — préfixe versionné permettant la
//! détection des valeurs chiffrées et une éventuelle rotation de schéma.

use aes_gcm::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use sha2::{Digest, Sha256};

use crate::requirement;

/// Préfixe des valeurs chiffrées (version 1 : AES-256-GCM, nonce 96 bits).
pub const ENC_PREFIX: &str = "enc:v1:";

/// Dérive la clé AES-256 depuis la valeur d'`ENCRYPTION_KEY` (chaîne libre, non vide).
/// SHA-256 : ergonomique pour l'opérateur (pas de format imposé), sans perte de sécurité par
/// rapport à la passphrase elle-même. `None` si la chaîne est vide ou blanche.
#[must_use]
#[requirement(REQ-SEC-004)]
pub fn derive_key(raw: &str) -> Option<[u8; 32]> {
    if raw.trim().is_empty() {
        return None;
    }
    Some(Sha256::digest(raw.as_bytes()).into())
}

/// Chiffre une valeur secrète, **liée à son contexte** par les données additionnelles
/// authentifiées (`aad`, revue SEC-004 F2) : un texte chiffré copié vers un autre foyer, un
/// autre type de canal ou un autre champ échoue à l'authentification GCM au déchiffrement —
/// pas de rejeu inter-contextes même avec un accès DB en écriture. Le nonce (96 bits) est tiré
/// aléatoirement à chaque appel : deux chiffrements de la même valeur produisent des sorties
/// différentes. `None` seulement si la primitive échoue (jamais en pratique) — l'appelant traite
/// ce cas comme une erreur interne, jamais comme un stockage en clair.
#[must_use]
#[requirement(REQ-SEC-004)]
pub fn encrypt(key: &[u8; 32], plaintext: &str, aad: &str) -> Option<String> {
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let payload = Payload {
        msg: plaintext.as_bytes(),
        aad: aad.as_bytes(),
    };
    let ciphertext = cipher.encrypt(&nonce, payload).ok()?;
    Some(format!(
        "{ENC_PREFIX}{}:{}",
        BASE64.encode(nonce),
        BASE64.encode(ciphertext)
    ))
}

/// Vrai si la valeur porte le préfixe d'une valeur chiffrée.
#[must_use]
#[requirement(REQ-SEC-004)]
pub fn is_encrypted(value: &str) -> bool {
    value.starts_with(ENC_PREFIX)
}

/// Déchiffre une valeur produite par [`encrypt`]. `None` si le format est illisible, la clé
/// mauvaise, ou le texte chiffré altéré (GCM authentifie : toute altération est détectée).
#[must_use]
#[requirement(REQ-SEC-004)]
pub fn decrypt(key: &[u8; 32], stored: &str, aad: &str) -> Option<String> {
    let rest = stored.strip_prefix(ENC_PREFIX)?;
    let (nonce_b64, ciphertext_b64) = rest.split_once(':')?;
    let nonce_bytes = BASE64.decode(nonce_b64).ok()?;
    let ciphertext = BASE64.decode(ciphertext_b64).ok()?;
    if nonce_bytes.len() != 12 {
        return None;
    }
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key));
    let payload = Payload {
        msg: ciphertext.as_slice(),
        aad: aad.as_bytes(),
    };
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), payload)
        .ok()?;
    String::from_utf8(plaintext).ok()
}

/// Contexte d'un secret de canal pour l'AAD : `(foyer, type de canal, champ)` — les trois axes
/// entre lesquels un rejeu de texte chiffré doit échouer (revue SEC-004 F2).
#[must_use]
#[requirement(REQ-SEC-004)]
pub fn channel_secret_aad(household_id: &str, kind: &str, field: &str) -> String {
    format!("{household_id}:{kind}:{field}")
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    fn key() -> [u8; 32] {
        derive_key("clef-de-test").unwrap()
    }

    #[test]
    #[verifies(REQ-SEC-004, case = "aller-retour chiffrement/déchiffrement, nonce unique par appel")]
    fn roundtrip_with_fresh_nonce() {
        let k = key();
        let a = encrypt(&k, "s3cr3t", "ctx").unwrap();
        let b = encrypt(&k, "s3cr3t", "ctx").unwrap();
        assert!(is_encrypted(&a));
        // Nonce aléatoire : même clair, sorties différentes (pas d'oracle d'égalité en base).
        assert_ne!(a, b);
        assert!(!a.contains("s3cr3t"));
        assert_eq!(decrypt(&k, &a, "ctx").as_deref(), Some("s3cr3t"));
        assert_eq!(decrypt(&k, &b, "ctx").as_deref(), Some("s3cr3t"));
    }

    #[test]
    #[verifies(REQ-SEC-004, case = "mauvaise clé, altération ou format illisible -> None (GCM authentifie)")]
    fn wrong_key_or_tampering_fails_closed() {
        let k = key();
        let stored = encrypt(&k, "s3cr3t", "ctx").unwrap();
        let other = derive_key("autre-clef").unwrap();
        assert_eq!(decrypt(&other, &stored, "ctx"), None);
        // Altération du dernier caractère du texte chiffré.
        let mut tampered = stored.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'A' { 'B' } else { 'A' });
        assert_eq!(decrypt(&k, &tampered, "ctx"), None);
        assert_eq!(decrypt(&k, "pas-chiffré", "ctx"), None);
        assert_eq!(decrypt(&k, "enc:v1:mauvais-format", "ctx"), None);
    }

    #[test]
    #[verifies(REQ-SEC-004, case = "AAD : un texte chiffré rejoué dans un autre contexte échoue (revue F2)")]
    fn ciphertext_is_bound_to_its_context() {
        let k = key();
        let aad = channel_secret_aad("foyer-a", "telegram", "bot_token");
        let stored = encrypt(&k, "s3cr3t", &aad).unwrap();
        assert_eq!(decrypt(&k, &stored, &aad).as_deref(), Some("s3cr3t"));
        // Autre foyer, autre type, autre champ : l'authentification GCM échoue.
        for other in [
            channel_secret_aad("foyer-b", "telegram", "bot_token"),
            channel_secret_aad("foyer-a", "gotify", "bot_token"),
            channel_secret_aad("foyer-a", "telegram", "token"),
        ] {
            assert_eq!(decrypt(&k, &stored, &other), None);
        }
    }

    #[test]
    #[verifies(REQ-SEC-004, case = "dérivation de clé : chaîne vide refusée, déterministe sinon")]
    fn key_derivation_rules() {
        assert_eq!(derive_key(""), None);
        assert_eq!(derive_key("   "), None);
        assert_eq!(derive_key("abc"), derive_key("abc"));
        assert_ne!(derive_key("abc"), derive_key("abd"));
    }
}
