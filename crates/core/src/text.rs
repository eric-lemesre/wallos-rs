//! Repli de texte pour la recherche (REQ-SUB-007).
//!
//! La recherche d'abonnements est **insensible à la casse et aux diacritiques** : « Été » doit
//! correspondre à « ete », « Café » à « cafe ». Le repli décompose la chaîne en forme **NFD**
//! (les caractères accentués se scindent en lettre de base + marque combinante), supprime les
//! marques combinantes, puis met en minuscule. Le périmètre visé est l'écriture latine (langues
//! `en`/`fr`) : les diacritiques du bloc *Combining Diacritical Marks* (U+0300..=U+036F) couvrent
//! é, è, ê, ë, à, ç, ñ, ü, å… Les lettres sans forme décomposée (ø, ß) sont conservées telles quelles
//! (repliées en minuscule seulement) — limitation assumée, sans incidence sur en/fr.

use unicode_normalization::UnicodeNormalization;
use wallos_req_macros::requirement;

/// Vrai si `c` est une marque combinante de diacritique (bloc U+0300..=U+036F).
///
/// Ce bloc rassemble les accents combinants de l'écriture latine, seuls produits par la décomposition
/// NFD des caractères accentués usuels (é → e + U+0301, ç → c + U+0327, etc.).
const fn is_combining_diacritic(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}')
}

/// Replie une chaîne pour comparaison de recherche : décomposition NFD, suppression des diacritiques
/// combinants, passage en minuscule.
///
/// Idempotente (`fold(fold(s)) == fold(s)`) et sans allocation superflue au-delà de la chaîne
/// résultat. La comparaison de recherche se fait ensuite par `contains` sur les formes repliées :
/// `fold(nom).contains(&fold(requête))`.
#[must_use]
#[requirement(REQ-SUB-007)]
pub fn fold_for_search(input: &str) -> String {
    input
        .nfd()
        .filter(|c| !is_combining_diacritic(*c))
        .flat_map(char::to_lowercase)
        .collect()
}

/// Vrai si `haystack` contient `needle` en comparaison repliée (casse + diacritiques ignorées).
///
/// Une requête vide correspond à tout (aucun filtrage). Le repli est appliqué aux deux opérandes,
/// garantissant la symétrie accent/casse dans les deux sens.
#[must_use]
#[requirement(REQ-SUB-007)]
pub fn matches_search(haystack: &str, needle: &str) -> bool {
    let needle = fold_for_search(needle);
    if needle.is_empty() {
        return true;
    }
    fold_for_search(haystack).contains(&needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_case_and_diacritics() {
        assert_eq!(fold_for_search("Été"), "ete");
        assert_eq!(fold_for_search("Café"), "cafe");
        assert_eq!(fold_for_search("Netflix"), "netflix");
        assert_eq!(fold_for_search("Señor Ñandú"), "senor nandu");
    }

    #[test]
    fn fold_is_idempotent() {
        let once = fold_for_search("Disney+ Éducation Çà");
        assert_eq!(fold_for_search(&once), once);
    }

    #[test]
    fn search_ignores_case_and_accents_both_ways() {
        // La requête accentuée trouve un nom non accentué et réciproquement.
        assert!(matches_search("Spotify Musique", "musique"));
        assert!(matches_search("Education nationale", "éducation"));
        assert!(matches_search("Éducation nationale", "education"));
    }

    #[test]
    fn empty_query_matches_everything() {
        assert!(matches_search("n'importe quoi", ""));
        assert!(matches_search("", ""));
    }

    #[test]
    fn non_matching_query_is_rejected() {
        assert!(!matches_search("Netflix", "spotify"));
        assert!(!matches_search("", "quelque chose"));
    }
}
