//! Politique de mot de passe (REQ-AUT-003).
//!
//! Aligné OWASP : longueur minimale plutôt que règles de composition arbitraires, et rejet des
//! mots de passe figurant dans une liste de compromis **embarquée** (aucune I/O — conforme à la
//! contrainte « zéro I/O » de `core`).

use std::collections::HashSet;
use std::sync::LazyLock;

use wallos_req_macros::requirement;

/// Longueur minimale d'un mot de passe (en points de code).
pub const MIN_PASSWORD_LENGTH: usize = 12;

/// Mots de passe compromis / trop courants (en minuscules). Extrait curé des classements de
/// fuites publiques. Les entrées < 12 caractères sont déjà rejetées par la longueur ; les entrées
/// >= 12 sont l'intérêt réel de ce contrôle.
const COMPROMISED_LIST: &[&str] = &[
    "password",
    "password1",
    "password12",
    "password123",
    "password1234",
    "passwordpassword",
    "123456789012",
    "1234567890123",
    "12345678901234",
    "qwertyuiop",
    "qwertyuiop123",
    "qwerty123456",
    "azertyuiop123",
    "administrator",
    "adminadmin123",
    "letmeinplease",
    "welcome123456",
    "iloveyouforever",
    "trustno1trustno1",
    "superman12345",
    "batman1234567",
    "football123456",
    "baseball123456",
    "dragon1234567",
    "monkey1234567",
    "sunshine12345",
    "princess12345",
    "whatever12345",
    "starwars12345",
    "computer12345",
];

/// Ensemble des mots de passe compromis, construit paresseusement depuis [`COMPROMISED_LIST`].
static COMPROMISED: LazyLock<HashSet<&'static str>> =
    LazyLock::new(|| COMPROMISED_LIST.iter().copied().collect());

/// Motif d'échec de la politique de mot de passe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PasswordPolicyError {
    /// Mot de passe plus court que [`MIN_PASSWORD_LENGTH`].
    #[error("password shorter than {MIN_PASSWORD_LENGTH} characters")]
    TooShort,
    /// Mot de passe présent dans la liste de compromis embarquée.
    #[error("password appears in the compromised list")]
    Compromised,
}

impl PasswordPolicyError {
    /// Clé i18n stable du message, partagée avec le frontend (REQ-I18N-002).
    #[must_use]
    #[requirement(REQ-AUT-003)]
    pub const fn message_key(self) -> &'static str {
        match self {
            Self::TooShort => "signup.validation.passwordTooShort",
            Self::Compromised => "signup.validation.passwordCompromised",
        }
    }
}

/// Valide un mot de passe candidat.
///
/// # Errors
/// - [`PasswordPolicyError::TooShort`] si la longueur est inférieure à [`MIN_PASSWORD_LENGTH`] ;
/// - [`PasswordPolicyError::Compromised`] s'il figure dans la liste de compromis embarquée.
#[requirement(REQ-AUT-003)]
pub fn validate_password(password: &str) -> Result<(), PasswordPolicyError> {
    if password.chars().count() < MIN_PASSWORD_LENGTH {
        return Err(PasswordPolicyError::TooShort);
    }
    if COMPROMISED.contains(password.to_lowercase().as_str()) {
        return Err(PasswordPolicyError::Compromised);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-AUT-003)]
    fn rejects_password_below_minimum_length() {
        assert_eq!(
            validate_password("short"),
            Err(PasswordPolicyError::TooShort)
        );
    }

    #[test]
    #[verifies(REQ-AUT-003)]
    fn accepts_password_at_exact_minimum_length() {
        // 12 caractères, absent de la liste de compromis.
        assert_eq!(validate_password("abcdefghijkl"), Ok(()));
    }

    #[test]
    #[verifies(REQ-AUT-003)]
    fn rejects_compromised_password_even_when_long_enough() {
        // 12 caractères mais présent dans la liste : la longueur ne suffit pas.
        assert_eq!(
            validate_password("password1234"),
            Err(PasswordPolicyError::Compromised)
        );
    }

    #[test]
    #[verifies(REQ-AUT-003)]
    fn compromised_check_is_case_insensitive() {
        assert_eq!(
            validate_password("PASSWORDPASSWORD"),
            Err(PasswordPolicyError::Compromised)
        );
    }

    #[test]
    #[verifies(REQ-AUT-003)]
    fn accepts_a_strong_passphrase() {
        assert_eq!(validate_password("correct horse battery staple"), Ok(()));
    }

    #[test]
    #[verifies(REQ-AUT-003)]
    fn error_message_keys_are_distinct_and_stable() {
        assert_eq!(
            PasswordPolicyError::TooShort.message_key(),
            "signup.validation.passwordTooShort"
        );
        assert_eq!(
            PasswordPolicyError::Compromised.message_key(),
            "signup.validation.passwordCompromised"
        );
    }
}
