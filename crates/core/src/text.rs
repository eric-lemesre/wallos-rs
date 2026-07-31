//! Repli de texte pour la recherche (REQ-SUB-007).
//!
//! La recherche d'abonnements est **insensible à la casse et aux diacritiques** : « Été » doit
//! correspondre à « ete », « Café » à « cafe ». Le repli décompose la chaîne en forme **NFD**
//! (les caractères accentués se scindent en lettre de base + marque combinante), supprime les
//! marques combinantes, met en minuscule, puis **développe les ligatures latines** (`œ`→`oe`,
//! `æ`→`ae`). Le périmètre visé est l'écriture latine (langues `en`/`fr`) : les diacritiques du bloc
//! *Combining Diacritical Marks* (U+0300..=U+036F) couvrent é, è, ê, ë, à, ç, ñ, ü, å… et les ligatures
//! `œ`/`æ` (cœur, œuvre, ex æquo) sont rendues cherchables via « oeur »/« ae ».
//!
//! **Hors périmètre (limitation assumée, sans incidence en/fr)** : les lettres non latines et les
//! caractères latins sans décomposition NFD ni ligature connue restent inchangés hors minuscule —
//! notamment le `ß` allemand (non replié en « ss ») et le `ø` nordique (non replié en « o »). La casse
//! suit `char::to_lowercase` (règles Unicode par défaut, sans traitement spécifique du turc `İ`/`ı`).

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
    let mut out = String::with_capacity(input.len());
    for base in input.nfd().filter(|c| !is_combining_diacritic(*c)) {
        for lower in base.to_lowercase() {
            // Ligatures latines développées après passage en minuscule (Œ/Æ → œ/æ via to_lowercase).
            match lower {
                'œ' => out.push_str("oe"),
                'æ' => out.push_str("ae"),
                other => out.push(other),
            }
        }
    }
    out
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
    fn expands_latin_ligatures() {
        // Ligatures françaises rendues cherchables sans ligature (revue kimi F2).
        assert_eq!(fold_for_search("Cœur"), "coeur");
        assert_eq!(fold_for_search("Œuvre"), "oeuvre");
        assert_eq!(fold_for_search("ex æquo"), "ex aequo");
        assert!(matches_search("Cœur de Pirate", "coeur"));
        assert!(matches_search("Ex Æquo", "aequo"));
    }

    #[test]
    fn out_of_scope_letters_are_only_lowercased() {
        // Limitation ASSUMÉE (périmètre en/fr) : ß et ø ne sont PAS repliés en « ss »/« o ».
        // Test de garde : si le repli est un jour étendu, cette attente devra être révisée sciemment.
        assert_eq!(fold_for_search("Straße"), "straße");
        assert!(!matches_search("Straße", "strasse"));
        assert_eq!(fold_for_search("København"), "københavn");
        assert!(!matches_search("København", "kobenhavn"));
    }

    #[test]
    fn fold_is_idempotent() {
        let once = fold_for_search("Disney+ Éducation Çà Cœur ex Æquo");
        assert_eq!(fold_for_search(&once), once);
    }

    #[test]
    fn leading_and_trailing_spaces_are_significant() {
        // `fold_for_search`/`matches_search` ne trime PAS : l'appelant (handler, UI) s'en charge.
        // Test de garde documentant ce contrat.
        assert!(!matches_search("Café", "  cafe"));
        assert!(matches_search("mon café serré", " café "));
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
