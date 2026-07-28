//! Référentiel des devises supportées (REQ-CUR-007).
//!
//! Un code devise libre ouvre la porte aux données incohérentes : la validation d'un montant se fait
//! **contre ce référentiel**. La liste est **capturée** sur l'application d'origine (table
//! `currencies` de Wallos 5.4.2, 34 devises — voir `e2e/fixtures/oracles/REQ-CUR-007-currencies.json`) ;
//! le nombre de décimales suit les **unités mineures ISO 4217** (0 pour JPY/KRW/ISK, 2 sinon — Wallos
//! ne stocke pas les décimales). Le `name` est le libellé par défaut (anglais) ; la localisation
//! relève de l'i18n (REQ-I18N-001), l'UI affichant le nom traduit par code.

use wallos_req_macros::requirement;

/// Une devise supportée : code ISO 4217, symbole, libellé par défaut, nombre de décimales d'affichage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportedCurrency {
    /// Code ISO 4217 à trois lettres.
    pub code: &'static str,
    /// Symbole d'affichage.
    pub symbol: &'static str,
    /// Libellé par défaut (anglais ; localisé par l'UI via i18n).
    pub name: &'static str,
    /// Nombre de décimales d'affichage (unités mineures ISO 4217), fourni à l'arrondi (REQ-CUR-005).
    pub decimals: u32,
}

/// Référentiel figé des devises supportées (capturé sur l'application d'origine).
const SUPPORTED: &[SupportedCurrency] = &[
    SupportedCurrency {
        code: "EUR",
        symbol: "€",
        name: "Euro",
        decimals: 2,
    },
    SupportedCurrency {
        code: "USD",
        symbol: "$",
        name: "US Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "JPY",
        symbol: "¥",
        name: "Japanese Yen",
        decimals: 0,
    },
    SupportedCurrency {
        code: "BGN",
        symbol: "лв",
        name: "Bulgarian Lev",
        decimals: 2,
    },
    SupportedCurrency {
        code: "CZK",
        symbol: "Kč",
        name: "Czech Republic Koruna",
        decimals: 2,
    },
    SupportedCurrency {
        code: "DKK",
        symbol: "kr",
        name: "Danish Krone",
        decimals: 2,
    },
    SupportedCurrency {
        code: "GBP",
        symbol: "£",
        name: "British Pound Sterling",
        decimals: 2,
    },
    SupportedCurrency {
        code: "HUF",
        symbol: "Ft",
        name: "Hungarian Forint",
        decimals: 2,
    },
    SupportedCurrency {
        code: "PLN",
        symbol: "zł",
        name: "Polish Zloty",
        decimals: 2,
    },
    SupportedCurrency {
        code: "RON",
        symbol: "lei",
        name: "Romanian Leu",
        decimals: 2,
    },
    SupportedCurrency {
        code: "SEK",
        symbol: "kr",
        name: "Swedish Krona",
        decimals: 2,
    },
    SupportedCurrency {
        code: "CHF",
        symbol: "Fr",
        name: "Swiss Franc",
        decimals: 2,
    },
    SupportedCurrency {
        code: "ISK",
        symbol: "kr",
        name: "Icelandic Króna",
        decimals: 0,
    },
    SupportedCurrency {
        code: "NOK",
        symbol: "kr",
        name: "Norwegian Krone",
        decimals: 2,
    },
    SupportedCurrency {
        code: "RUB",
        symbol: "₽",
        name: "Russian Ruble",
        decimals: 2,
    },
    SupportedCurrency {
        code: "TRY",
        symbol: "₺",
        name: "Turkish Lira",
        decimals: 2,
    },
    SupportedCurrency {
        code: "AUD",
        symbol: "$",
        name: "Australian Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "BRL",
        symbol: "R$",
        name: "Brazilian Real",
        decimals: 2,
    },
    SupportedCurrency {
        code: "CAD",
        symbol: "$",
        name: "Canadian Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "CNY",
        symbol: "¥",
        name: "Chinese Yuan",
        decimals: 2,
    },
    SupportedCurrency {
        code: "HKD",
        symbol: "HK$",
        name: "Hong Kong Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "IDR",
        symbol: "Rp",
        name: "Indonesian Rupiah",
        decimals: 2,
    },
    SupportedCurrency {
        code: "ILS",
        symbol: "₪",
        name: "Israeli New Sheqel",
        decimals: 2,
    },
    SupportedCurrency {
        code: "INR",
        symbol: "₹",
        name: "Indian Rupee",
        decimals: 2,
    },
    SupportedCurrency {
        code: "KRW",
        symbol: "₩",
        name: "South Korean Won",
        decimals: 0,
    },
    SupportedCurrency {
        code: "MXN",
        symbol: "Mex$",
        name: "Mexican Peso",
        decimals: 2,
    },
    SupportedCurrency {
        code: "MYR",
        symbol: "RM",
        name: "Malaysian Ringgit",
        decimals: 2,
    },
    SupportedCurrency {
        code: "NZD",
        symbol: "NZ$",
        name: "New Zealand Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "PHP",
        symbol: "₱",
        name: "Philippine Peso",
        decimals: 2,
    },
    SupportedCurrency {
        code: "SGD",
        symbol: "S$",
        name: "Singapore Dollar",
        decimals: 2,
    },
    SupportedCurrency {
        code: "THB",
        symbol: "฿",
        name: "Thai Baht",
        decimals: 2,
    },
    SupportedCurrency {
        code: "ZAR",
        symbol: "R",
        name: "South African Rand",
        decimals: 2,
    },
    SupportedCurrency {
        code: "UAH",
        symbol: "₴",
        name: "Ukrainian Hryvnia",
        decimals: 2,
    },
    SupportedCurrency {
        code: "TWD",
        symbol: "NT$",
        name: "New Taiwan Dollar",
        decimals: 2,
    },
];

/// Toutes les devises supportées, dans l'ordre du référentiel.
#[must_use]
#[requirement(REQ-CUR-007)]
pub fn all() -> &'static [SupportedCurrency] {
    SUPPORTED
}

/// Cherche une devise par son code. `None` si le code n'est pas dans le référentiel supporté.
#[must_use]
#[requirement(REQ-CUR-007)]
pub fn find(code: &str) -> Option<&'static SupportedCurrency> {
    SUPPORTED.iter().find(|c| c.code == code)
}

/// Vrai si le code appartient au référentiel supporté (validation métier).
#[must_use]
#[requirement(REQ-CUR-007)]
pub fn is_supported(code: &str) -> bool {
    find(code).is_some()
}

/// Nombre de décimales d'affichage d'une devise, ou `None` si non supportée (source de l'arrondi
/// REQ-CUR-005 et du formatage REQ-CUR-006).
#[must_use]
#[requirement(REQ-CUR-007)]
pub fn decimals(code: &str) -> Option<u32> {
    find(code).map(|c| c.decimals)
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-CUR-007, case = "référentiel capturé (34 devises, nom/symbole/décimales)")]
    fn referential_matches_captured_oracle() {
        // Oracle figé : e2e/fixtures/oracles/REQ-CUR-007-currencies.json.
        assert_eq!(all().len(), 34);
        let eur = find("EUR").unwrap();
        assert_eq!(eur.symbol, "€");
        assert_eq!(eur.name, "Euro");
        assert_eq!(eur.decimals, 2);
        // Devises sans sous-unité (ISO 4217) : 0 décimale.
        assert_eq!(decimals("JPY"), Some(0));
        assert_eq!(decimals("KRW"), Some(0));
        assert_eq!(decimals("ISK"), Some(0));
        assert_eq!(find("JPY").unwrap().symbol, "¥");
    }

    #[test]
    #[verifies(REQ-CUR-007, case = "code hors référentiel rejeté")]
    fn unsupported_code_is_rejected() {
        assert!(!is_supported("ZZZ"));
        assert!(find("ZZZ").is_none());
        assert_eq!(decimals("ZZZ"), None);
        // Un code supporté passe.
        assert!(is_supported("USD"));
    }

    #[test]
    #[verifies(REQ-CUR-007, case = "codes uniques et bien formés")]
    fn codes_are_unique_and_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for c in all() {
            assert_eq!(c.code.len(), 3, "code {} n'a pas 3 lettres", c.code);
            assert!(c.code.chars().all(|ch| ch.is_ascii_uppercase()));
            assert!(!c.symbol.is_empty());
            assert!(!c.name.is_empty());
            assert!(seen.insert(c.code), "code dupliqué: {}", c.code);
        }
    }
}
