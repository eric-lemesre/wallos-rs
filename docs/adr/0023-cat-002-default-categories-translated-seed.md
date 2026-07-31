# ADR 0023 — Catégories par défaut : seed traduit + sentinelle exclue (extension de l'oracle legacy)

- **Statut** : accepté (2026-07-31)
- **Contexte** : REQ-CAT-002 (catégories par défaut à la création du compte), exigence `oracle: legacy`,
  criticality low, layer `[core, api]`, dépend de CAT-001 ✓ + I18N-001 ✓.

## Problème

Un compte fraîchement créé arrivait avec une liste de catégories **vide**. REQ-CAT-002 demande qu'un
**jeu de catégories par défaut** soit présent à la création, **dans la langue du compte**.

La **capture** de l'oracle (Wallos 5.4.2, `registration.php` : tableau `$categories` + boucle d'INSERT
`INSERT INTO categories (name, "order", user_id)`) révèle deux frictions avec la spec et avec le modèle
de données de subtrack :

1. Wallos sème **17 catégories en anglais littéral**, quelle que soit la langue choisie (le champ
   `language` est stocké sur l'utilisateur mais **n'est pas appliqué au seed** ; la traduction n'existe
   qu'à la demande via `endpoints/ai/translate_categories.php`). Il n'y a donc **aucune traduction FR
   canonique** dans le legacy — alors que l'acceptance CAT-002 exige explicitement « dans la langue du
   compte » + « traduite » (d'où la dépendance à I18N-001).
2. La 1ʳᵉ catégorie legacy est une **sentinelle** « No category » (id=1, protégée contre la suppression).
   subtrack modélise déjà « sans catégorie » par un `category_id` **NULL** (libellé UI « (aucune) »), pas
   par une ligne réelle.

## Décision

**On étend délibérément le comportement legacy, en documentant les deux divergences dans la fixture
oracle `e2e/fixtures/oracles/REQ-CAT-002-default-categories.json`.**

1. **Seed traduit selon la langue du compte.** Le jeu par défaut est porté en dur dans `core`
   (`default_category_names(Language) -> [&str; 16]`, source de vérité en/fr) et semé dans la
   **transaction de création de compte** (`UserRepository::create_account`), atomiquement avec le foyer
   et l'utilisateur. Comme `createAccount` ne collectait aucune langue (le formulaire d'inscription de
   Wallos, lui, a un sélecteur de langue), on **ajoute un champ `language` optionnel** à
   `CreateAccountRequest` : présent et supporté ⇒ langue persistée sur l'utilisateur + jeu traduit ;
   présent non supporté ⇒ **422** (cohérent avec `PUT /settings/language`) ; absent ⇒ colonne `language`
   à `NULL` (repli langue système côté UI) + jeu par défaut **anglais** (langue de base). Les traductions
   FR sont **propres à subtrack** (le legacy n'en fournit aucune).

2. **Sentinelle exclue → 16 catégories.** On ne sème **pas** « No category » : « sans catégorie » reste
   exclusivement `category_id NULL`. Cela évite la redondance NULL/ligne (et l'ambiguïté pour les stats
   par catégorie à venir) et **ne rouvre pas** la dette de protection d'une ligne sentinelle, que CAT-003
   avait explicitement différée (cf. `REQ-CAT-003-category-delete.json`, `default_category_protected`).

## Conséquences

- **Baseline de test modifiée** : chaque compte naît désormais avec 16 catégories. Les tests
  CRUD/isolation de `crates/server/tests/categories.rs` raisonnent via un helper `custom_categories`
  (catégories **hors** jeu par défaut), pour préserver leur intention d'origine sans coupler chaque
  assertion à la liste par défaut.
- **UI hors périmètre** : CAT-002 est `[core, api]`. Le formulaire d'inscription web n'expose pas encore
  de sélecteur de langue ; le champ `language` est optionnel et rétro-compatible. Un branchement UI
  (transmettre la langue navigateur au signup) pourra suivre sans nouvelle décision.
- **Ordre non persisté** : conformément à CAT-005 (déjà figé, ordre alphabétique déterministe au
  listage), l'ordre d'insertion legacy (« No category » en tête) n'est pas reproduit — sans portée pour
  l'acceptance CAT-002, qui ne demande que la **présence** du jeu.
- **e2e** : `e2e: optional` pour CAT-002 ; la vérification vit au niveau `core` (unitaire) + `api`
  (intégration storage & serveur). Les specs e2e existantes vérifient des noms de catégories
  spécifiques et restent robustes au seed.
