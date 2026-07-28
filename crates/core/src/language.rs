//! Langue de l'interface et des notifications (REQ-I18N-001).
//!
//! La langue est un **choix de l'utilisateur persisté côté serveur** : elle conditionne aussi le
//! contenu des notifications émises par le serveur, elle ne peut donc pas être un simple réglage local
//! du navigateur. Ce type est le **référentiel des langues supportées** : une langue non supportée est
//! refusée au niveau du domaine (acceptance #2 : la langue système n'est appliquée que si supportée).

use wallos_req_macros::requirement;

use crate::DomainError;

/// Une langue supportée par l'application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// Anglais.
    English,
    /// Français.
    French,
}

impl Language {
    /// Toutes les langues supportées, dans un ordre stable (aligné sur les ressources i18n de l'UI).
    pub const ALL: [Language; 2] = [Language::English, Language::French];

    /// Code court stable de la langue (`en`, `fr`) — clé des ressources i18n.
    #[must_use]
    #[requirement(REQ-I18N-001)]
    pub const fn code(self) -> &'static str {
        match self {
            Language::English => "en",
            Language::French => "fr",
        }
    }

    /// Analyse un code de langue ; **seules les langues supportées** sont acceptées.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le code n'est pas une langue supportée.
    #[requirement(REQ-I18N-001)]
    pub fn parse(code: &str) -> Result<Self, DomainError> {
        match code {
            "en" => Ok(Language::English),
            "fr" => Ok(Language::French),
            other => Err(DomainError::InvalidArgument(format!(
                "unsupported language: {other}"
            ))),
        }
    }

    /// Indique si un code de langue est supporté (acceptance #2 : repli système seulement si supporté).
    #[must_use]
    #[requirement(REQ-I18N-001)]
    pub fn is_supported(code: &str) -> bool {
        Self::parse(code).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-I18N-001, case = "code <-> parse pour chaque langue supportée")]
    fn code_round_trips_for_every_supported_language() {
        for lang in Language::ALL {
            assert_eq!(Language::parse(lang.code()).unwrap(), lang);
        }
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::French.code(), "fr");
    }

    #[test]
    #[verifies(REQ-I18N-001, case = "langue non supportée refusée")]
    fn unsupported_language_is_rejected() {
        assert!(Language::parse("de").is_err());
        assert!(Language::parse("EN").is_err()); // sensible à la casse (code canonique minuscule)
        assert!(!Language::is_supported("de"));
        assert!(Language::is_supported("fr"));
    }
}
