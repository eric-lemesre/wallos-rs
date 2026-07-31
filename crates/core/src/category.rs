//! Modèle de catégorie (REQ-CAT-001).
//!
//! Dimension d'analyse principale des statistiques. Entité **possédée** : l'isolation par foyer
//! (§9) est portée par le repository (`&Actor`), comme les autres entités ; le modèle pur ne porte
//! que l'identité métier (id + nom). Le nom est obligatoire (non vide).

use uuid::Uuid;
use wallos_req_macros::requirement;

use crate::DomainError;
use crate::language::Language;

/// Une catégorie d'abonnements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    id: Uuid,
    name: String,
}

/// Longueur maximale d'un nom de catégorie (caractères Unicode).
pub const NAME_MAX_LEN: usize = 100;

impl Category {
    /// Construit une catégorie. Le nom est **normalisé** (espaces de bord retirés) ; il ne doit pas
    /// être vide, ni dépasser [`NAME_MAX_LEN`] caractères.
    ///
    /// # Errors
    /// [`DomainError::InvalidArgument`] si le nom (après normalisation) est vide ou trop long.
    #[requirement(REQ-CAT-001)]
    pub fn new(id: Uuid, name: impl Into<String>) -> Result<Self, DomainError> {
        let name = name.into().trim().to_string();
        if name.is_empty() {
            return Err(DomainError::InvalidArgument(
                "category name must not be empty".to_string(),
            ));
        }
        if name.chars().count() > NAME_MAX_LEN {
            return Err(DomainError::InvalidArgument(format!(
                "category name exceeds {NAME_MAX_LEN} characters"
            )));
        }
        Ok(Self { id, name })
    }

    /// Identifiant stable.
    #[must_use]
    #[requirement(REQ-CAT-001)]
    pub const fn id(&self) -> Uuid {
        self.id
    }

    /// Nom de la catégorie.
    #[must_use]
    #[requirement(REQ-CAT-001)]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Forme **canonique** du nom (REQ-CAT-004) : normalisée en minuscules, à titre **illustratif** de
    /// la règle d'unicité insensible à la casse (« Streaming » et « streaming » sont homonymes).
    ///
    /// **L'enforcement fait autorité est l'index unique `(household_id, lower(name))` de la base** : c'est
    /// lui qui rejette les doublons. Ce helper core exprime la même intention côté domaine ; il peut
    /// diverger de `lower()` PostgreSQL sur des cas Unicode exotiques (I turc, NFC/NFD) — raison pour
    /// laquelle l'unicité est tranchée par la base (locale unique), jamais par une comparaison en `core`.
    #[must_use]
    #[requirement(REQ-CAT-004)]
    pub fn canonical_name(&self) -> String {
        self.name.to_lowercase()
    }
}

/// Politique de suppression d'une catégorie **référencée** (REQ-CAT-003).
///
/// **Comportement de référence capturé sur Wallos 5.4.2** (§8.1, `endpoints/categories/category.php`,
/// `handleDeleteCategory`) : avant toute suppression, Wallos compte les abonnements du compte qui
/// pointent vers la catégorie ; si ce compte est non nul, la suppression est **refusée** (message
/// `category_in_use`) — jamais de réaffectation à une catégorie par défaut, jamais de cascade sur les
/// abonnements. C'est précisément le piège que l'exigence pointe : un agent inventerait volontiers un
/// `SET NULL` ou un `CASCADE` plausibles mais divergents.
///
/// Renvoie `true` si la catégorie peut être supprimée (aucun abonnement ne la référence).
///
/// **Portée exacte de la garantie (CAT-003 #2)** : appliquée au chemin de suppression, cette politique
/// assure qu'*une suppression qui aboutit ne laisse jamais un abonnement pointer vers une catégorie
/// supprimée* — le critère « quand la suppression aboutit, aucun abonnement ne référence une catégorie
/// inexistante ». Elle ne prétend **pas** garantir l'intégrité référentielle du *chemin de création*
/// d'abonnement (qui, faute de clé étrangère — choix offline-first, ADR 0006/SYN-001 — ne vérifie pas
/// aujourd'hui l'existence de la catégorie liée) : ce durcissement relève d'une passe d'intégrité
/// référentielle dédiée, hors périmètre CAT-003 (cf. revue 2026-07-29-cat-003, F1).
#[must_use]
#[requirement(REQ-CAT-003)]
pub const fn category_is_deletable(referencing_subscriptions: u64) -> bool {
    referencing_subscriptions == 0
}

/// Nombre de catégories par défaut semées à la création d'un compte (REQ-CAT-002).
pub const DEFAULT_CATEGORY_COUNT: usize = 16;

/// Jeu de catégories par défaut, **dans la langue du compte** (REQ-CAT-002).
///
/// **Oracle capturé sur Wallos 5.4.2** (`registration.php`, tableau `$categories` + boucle d'INSERT) :
/// à l'inscription, Wallos sème un jeu de catégories fixe. Deux divergences **assumées et documentées**
/// (fixture `e2e/fixtures/oracles/REQ-CAT-002-default-categories.json`) :
///
/// 1. **Traduction** — Wallos sème les noms en **anglais littéral** quelle que soit la langue choisie
///    (la traduction n'existe qu'à la demande via un endpoint IA) ; la spec CAT-002 exige au contraire
///    « dans la langue du compte » (dépendance REQ-I18N-001). On traduit donc le jeu selon [`Language`].
/// 2. **Sentinelle exclue** — Wallos sème une 17ᵉ catégorie sentinelle « No category » (id=1, protégée) ;
///    subtrack modélise « sans catégorie » par un `category_id` **NULL** (libellé UI « (aucune) »), on
///    ne sème donc **que les 16 catégories substantielles**, pas la sentinelle.
///
/// L'ordre du tableau n'a pas de portée : la liste est restituée par ordre alphabétique déterministe
/// (REQ-CAT-005). Les deux langues sont alignées **index par index** (même cardinalité, cf. test).
#[must_use]
#[requirement(REQ-CAT-002)]
pub const fn default_category_names(language: Language) -> [&'static str; DEFAULT_CATEGORY_COUNT] {
    match language {
        Language::English => [
            "Entertainment",
            "Music",
            "Utilities",
            "Food & Beverages",
            "Health & Wellbeing",
            "Productivity",
            "Banking",
            "Transport",
            "Education",
            "Insurance",
            "Gaming",
            "News & Magazines",
            "Software",
            "Technology",
            "Cloud Services",
            "Charity & Donations",
        ],
        Language::French => [
            "Divertissement",
            "Musique",
            "Services publics",
            "Alimentation et boissons",
            "Santé et bien-être",
            "Productivité",
            "Banque",
            "Transport",
            "Éducation",
            "Assurance",
            "Jeux vidéo",
            "Actualités et magazines",
            "Logiciels",
            "Technologie",
            "Services cloud",
            "Dons et charité",
        ],
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    #[test]
    #[verifies(REQ-CAT-001, case = "nom obligatoire + accesseurs")]
    fn builds_and_exposes_fields() {
        let id = Uuid::from_u128(1);
        let c = Category::new(id, "Streaming").unwrap();
        assert_eq!(c.id(), id);
        assert_eq!(c.name(), "Streaming");
    }

    #[test]
    #[verifies(REQ-CAT-001, case = "nom vide refusé")]
    fn empty_name_is_rejected() {
        assert!(Category::new(Uuid::from_u128(1), "").is_err());
        assert!(Category::new(Uuid::from_u128(1), "   ").is_err());
    }

    #[test]
    #[verifies(REQ-SYN-001, case = "l'identifiant (UUID) fourni est conservé tel quel")]
    fn identity_uuid_is_preserved() {
        // Un UUID **généré côté client** (offline-first) est conservé à l'identique par le modèle :
        // rien ne le réécrit. Préalable à la réplication (REQ-SYN-001).
        let client_id = Uuid::from_u128(0xdead_beef);
        let c = Category::new(client_id, "Streaming").unwrap();
        assert_eq!(c.id(), client_id);
    }

    #[test]
    #[verifies(REQ-CAT-004, case = "nom canonique insensible à la casse")]
    fn canonical_name_is_case_insensitive() {
        let a = Category::new(Uuid::from_u128(1), "Streaming").unwrap();
        let b = Category::new(Uuid::from_u128(2), "streaming").unwrap();
        // Deux graphies de casse différente partagent la même forme canonique (homonymes).
        assert_eq!(a.canonical_name(), b.canonical_name());
        assert_eq!(a.canonical_name(), "streaming");
        // Un nom distinct a une forme canonique distincte.
        let c = Category::new(Uuid::from_u128(3), "Musique").unwrap();
        assert_ne!(a.canonical_name(), c.canonical_name());
    }

    #[test]
    #[verifies(REQ-CAT-003, case = "catégorie référencée non supprimable (oracle Wallos)")]
    fn referenced_category_is_not_deletable() {
        // Oracle figé : e2e/fixtures/oracles/REQ-CAT-003-category-delete.json (Wallos 5.4.2
        // handleDeleteCategory : compte > 0 -> `category_in_use`, la catégorie n'est PAS supprimée).
        assert!(category_is_deletable(0), "aucune référence -> supprimable");
        assert!(
            !category_is_deletable(1),
            "une référence -> non supprimable (jamais SET NULL ni CASCADE)"
        );
        assert!(
            !category_is_deletable(42),
            "plusieurs références -> toujours non supprimable"
        );
    }

    #[test]
    #[verifies(REQ-CAT-002, case = "jeu par défaut : cardinalité, validité et alignement des langues")]
    fn default_categories_are_well_formed_in_every_language() {
        for lang in Language::ALL {
            let names = default_category_names(lang);
            assert_eq!(
                names.len(),
                DEFAULT_CATEGORY_COUNT,
                "cardinalité stable ({DEFAULT_CATEGORY_COUNT}) pour {}",
                lang.code()
            );
            // Chaque nom est un nom de catégorie valide (non vide, borné, sans espaces de bord) :
            // il doit pouvoir être construit par la même politique que les catégories utilisateur.
            for name in names {
                let built = Category::new(Uuid::from_u128(1), name).unwrap();
                assert_eq!(built.name(), name, "nom déjà normalisé (aucun trim requis)");
            }
            // Aucun doublon (l'unicité par foyer, CAT-004, ne doit jamais rejeter un défaut).
            let mut sorted: Vec<&str> = names.to_vec();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(
                sorted.len(),
                names.len(),
                "noms distincts pour {}",
                lang.code()
            );
        }
        // Les traductions diffèrent bien de l'anglais là où c'est attendu (jeu réellement traduit).
        assert_eq!(default_category_names(Language::English)[1], "Music");
        assert_eq!(default_category_names(Language::French)[1], "Musique");
    }

    #[test]
    #[verifies(REQ-CAT-001, case = "nom normalisé (trim) et borné")]
    fn name_is_trimmed_and_length_bounded() {
        // Les espaces de bord sont retirés avant stockage (pas de "  X  " en base).
        let c = Category::new(Uuid::from_u128(1), "  Streaming  ").unwrap();
        assert_eq!(c.name(), "Streaming");
        // Un nom à la longueur maximale passe ; au-delà, il est refusé.
        assert!(Category::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN)).is_ok());
        assert!(Category::new(Uuid::from_u128(1), "a".repeat(NAME_MAX_LEN + 1)).is_err());
    }
}
