# Revue de code – cœur de domaine wallos-rs

**Date** : 2026-07-28  
**Périmètre** : `crates/core/src/{money,billing,subscription,currencies,exchange}.rs`, `crates/proto/src/lib.rs`, `crates/server/src/{exchange,currencies}.rs`  
**Référentiel** : `AGENTS.md` (R4, R5, §9), `spec/requirements/currencies.md` (CUR-002/003/004/005/007), `spec/requirements/subscriptions.md` (SUB-001/003)

## Résumé

Le domaine fait correctement le choix de `rust_decimal`, expose une volonté de traçabilité (`#[requirement]`, `#[verifies]`) et gère bien le mode dégradé des conversions. En revanche, il présente des failles réelles aux frontières : validation des devises insuffisante, routes API non versionnées, modèle de date inadapté aux exigences d'échéance, et absence d'opérations monétaires contrôlées. Les tests couvrent les cas nominaux mais laissent des angles morts sur la validation des codes devise, la casse, et la corruption des taux.

---

## Statut de traitement (mis à jour après triage)

Légende : ✅ CORRIGÉ · ❌ REJETÉ (faux positif) · ⏳ DIFFÉRÉ (exigence future / backlog `.hermes`).

| Finding | Statut | Détail |
|---|---|---|
| HIGH-1 (validation devise hors référentiel) | ✅ CORRIGÉ | `CurrencyCode::new` valide référentiel + majuscules — PR #31 (`374a341`). Test bout-en-bout `target=ZZZ → 422`. |
| HIGH-2 (routes sans `/api/v1`) | ❌ REJETÉ | Faux positif : routes nichées sous `/api/v1` via `Router::nest("/api/v1", …)` dans `lib.rs`. |
| HIGH-3 (`first_payment` `NaiveDate` sans heure/fuseau) | ⏳ DIFFÉRÉ | À trancher pour **REQ-SUB-012** (Wallos stocke une DATE ; réconcilier « heure locale préservée »). |
| HIGH-4 (modèle `Subscription` incomplet) | ⏳ DIFFÉRÉ | Étendre au fil de **SUB-009/010**, **NOT**, **SYN** (pas spéculativement). |
| HIGH-5 (pas d'`owner_id`) | ⏳ DIFFÉRÉ | À ajouter avec la persistance (**SUB-002**) ; repo filtrant par `Actor` (§9). Pas de repo abonnement encore → non exploitable. |
| MEDIUM-1 (minuscules acceptées) | ✅ CORRIGÉ | `is_ascii_uppercase` — PR #31 (`374a341`). |
| MEDIUM-2 (taux corrompus ignorés en silence) | ⏳ DIFFÉRÉ | Ajouter `tracing::warn` sur ligne ignorée (`.hermes`). |
| MEDIUM-3 (422 sans `detail`) | ⏳ DIFFÉRÉ | Enrichir `Problem.detail` (champ fautif) (`.hermes`). |
| MEDIUM-4 (`Money` sans `Add`/`Sum` contrôlé) | ⏳ DIFFÉRÉ | Ajouter `Money::add`→`Result` quand l'agrégation métier (**STA**) en aura besoin. |
| LOW-1 (`u32` vs `usize` compteurs) | ⏳ DIFFÉRÉ | `.hermes`. |
| LOW-2 (`Secret` ne zeroize pas la mémoire) | ⏳ DIFFÉRÉ | SEC-003 = masquage logs (satisfait) ; durcissement mémoire optionnel (`secrecy`/`zeroize`). |
| NIT-1 (`InvalidMoney` pour taux ≤0) | ⏳ DIFFÉRÉ | Taxonomie d'erreur (`InvalidArgument`/`InvalidRate`) (`.hermes`). |
| NIT-2 (dédup `rate`/`dated_rate`) | ⏳ DIFFÉRÉ | Refactor mineur `RateTable` (`.hermes`). |
| NIT-3 (`SubscriptionDto.active` sans défaut) | ✅ CORRIGÉ | `#[serde(default)]` (actif) — PR #31 (`374a341`). |

---

## Findings

### CRITICAL

Aucun finding de criticité CRITICAL dans le périmètre lu (pas de panic cachée, pas de fuite directe de secret dans les logs). Le finding **HIGH-1** ci-dessous touche une règle métier fondamentale et pourrait être reclassé CRITICAL si l'on considère l'intégrité de la dimension devise comme critique.

---

### HIGH

#### HIGH-1 — Validation devise désactivée : `CurrencyCode::new` accepte les codes hors référentiel
- **Fichier:ligne** : `crates/core/src/money.rs:88`, `crates/proto/src/lib.rs:307-316`
- **Défaut** : `CurrencyCode::new` ne vérifie que la longueur (3) et le caractère alphabétique. `SubscriptionDto::into_core` appelle `CurrencyCode::new` puis `Money::new`, mais ne consulte jamais `wallos_core::currencies::is_supported`. Résultat : un DTO avec `"currency": "ZZZ"` passe la conversion domaine.
- **Scénario d'échec concret** : Un client crée un abonnement avec `"currency": "XYZ"`. Le serveur accepte, stocke, puis l'agrégation multi-devises exclut silencieusement ce montant (`excluded += 1`, `complete = false`) sans message explicite. L'utilisateur voit un total plus bas sans comprendre pourquoi. Violation directe de REQ-CUR-007 : *« given: un code devise hors référentiel ISO 4217, when: il est soumis, then: la validation échoue côté serveur »*.
- **Correctif suggéré** : Faire échouer `CurrencyCode::new` (ou `Money::new`/`SubscriptionDto::into_core`) sur devise non supportée. Préférer valider dès `CurrencyCode::new` pour interdire la construction d'un code invalide quel que soit le chemin d'appel.

#### HIGH-2 — Routes API sans préfixe de version `/api/v1`
- **Fichier:ligne** : `crates/server/src/currencies.rs:16`, `crates/server/src/exchange.rs:128`
- **Défaut** : Les chemins sont `/currencies` et `/exchange/aggregate`. `AGENTS.md` §6 impose : *« Versionnement par préfixe `/api/v1`. Toute rupture de contrat exige un ADR. »*
- **Scénario d'échec concret** : Une future v2 doit modifier le schéma `AggregateRequest`. Sans préfixe v1, les clients existants sont cassés dès le premier changement. La porte `cargo xtask openapi --check` ne détectera pas l'absence de version si l'artefact généré reproduit le même oubli.
- **Correctif suggéré** : Préfixer les deux routes par `/api/v1`, régénérer `api/openapi.json`, et ajouter un test d'intégration qui vérifie que toute route serveur commence par `/api/v1`.

#### HIGH-3 — `first_payment` en `NaiveDate` ne peut pas préserver l'heure locale
- **Fichier:ligne** : `crates/core/src/subscription.rs:31`, `crates/core/src/subscription.rs:164-166`
- **Défaut** : La date de premier paiement est un `chrono::NaiveDate`, sans heure ni fuseau horaire. REQ-SUB-012 exige pourtant : *« given: un abonnement mensuel dont l'échéance tombe un jour de changement d'heure, when: on calcule la prochaine échéance, then: l'heure locale de facturation est préservée »*.
- **Scénario d'échec concret** : Un abonnement facturé à 02:30 heure locale voit son heure de prélèvement perdue ou décalée dès qu'intervient un changement d'heure. Le modèle actuel ne représente même pas l'heure, rendant le critère d'acceptation impossible à satisfier.
- **Correctif suggéré** : Remplacer `NaiveDate` par un type date-heure avec fuseau horaire (`chrono::DateTime<Tz>` ou `NaiveDateTime` + timezone explicite) dans le modèle d'abonnement et les DTO associés.

#### HIGH-4 — Modèle `Subscription` incomplet par rapport aux exigences futures et à la table Wallos
- **Fichier:ligne** : `crates/core/src/subscription.rs:26-39`
- **Défaut** : Le modèle couvre les champs de REQ-SUB-001, mais ignore les champs connus de Wallos et requis par d'autres exigences : date de fin/cancellation (REQ-SUB-009), fin d'essai (REQ-SUB-010), paramètres de notification, et timestamps de création/modification nécessaires à la synchro (REQ-SYN-005).
- **Scénario d'échec concret** : Lors de l'implémentation de SUB-009/010, le modèle doit être étendu avec migration de données. Si la base contient déjà des abonnements sans ces champs, la migration est coûteuse, et les oracles E2E préalablement enregistrés peuvent devenir inconsistents.
- **Correctif suggéré** : Étendre `Subscription` et `SubscriptionDto` avec les champs identifiés (même si leur logique métier vient plus tard) pour stabiliser le schéma.

#### HIGH-5 — `Subscription` ne porte pas d'`owner_id`
- **Fichier:ligne** : `crates/core/src/subscription.rs:26-39`
- **Défaut** : `AGENTS.md` §9 impose que *« toute entité métier porte `owner_id` non nullable »*. Le modèle `Subscription` n'a aucun champ de propriétaire/foyer.
- **Scénario d'échec concret** : L'isolation repose entièrement sur la couche `storage`/`server`. Une erreur dans un repository (par exemple `SELECT * FROM subscriptions WHERE id = $1` sans clause `owner_id`) expose les abonnements d'un autre utilisateur. Le type ne rend pas l'oubli impossible à compiler, contrairement au garde-fou structurale décrit en §9.
- **Correctif suggéré** : Ajouter `owner_id: Uuid` au modèle domaine, ou documenter explicitement (ADR) pourquoi `core` en est exempté et comment `storage` garantit l'isolation de manière irrévocable.

---

### MEDIUM

#### MEDIUM-1 — `CurrencyCode` accepte les minuscules, cassant l'égalité et les lookups
- **Fichier:ligne** : `crates/core/src/money.rs:90`
- **Défaut** : `bytes.iter().all(|b| b.is_ascii_alphabetic())` autorise `"eur"`. Deux codes `"eur"` et `"EUR"` représentent la même devise mais ne sont pas égaux (`CurrencyCode` dérive `Eq` sur les octets bruts). De plus, `currencies::find` fait une comparaison sensible à la casse.
- **Scénario d'échec concret** : Un import de données ou un client envoie `"eur"`. `CurrencyCode::new("eur")` réussit, mais `currencies::find("eur")` retourne `None`. L'agrégation marque le montant comme exclu et le total devient partiel sans message clair.
- **Correctif suggéré** : Forcer la mise en majuscule dans `CurrencyCode::new` et valider `is_ascii_uppercase()`.

#### MEDIUM-2 — `load_rate_table` ignore silencieusement les taux stockés corrompus
- **Fichier:ligne** : `crates/server/src/exchange.rs:91-106`
- **Défaut** : Les lignes avec `rate <= 0` ou codes devise invalides sont passées (`continue`) sans log, sans métrique, sans erreur.
- **Scénario d'échec concret** : Une corruption partielle de la table `exchange_rates` (bug de migration, fournisseur défectueux) passe inaperçue. L'utilisateur voit un agrégat partiel sans savoir qu'un taux manque, ce qui contredit l'esprit de REQ-CUR-004 (signal explicite).
- **Correctif suggéré** : Journaliser au minimum en `warn` chaque ligne ignorée avec la raison, ou remonter une erreur si le taux faisait partie de la paire demandée.

#### MEDIUM-3 — `aggregate_converted_handler` retourne un `Problem` 422 sans détail
- **Fichier:ligne** : `crates/server/src/exchange.rs:156-170`
- **Défaut** : Que le `target` soit invalide ou qu'un `amount` soit illisible, la réponse est `problem(422, "about:blank", "Unprocessable Entity")` sans champ `detail`.
- **Scénario d'échec concret** : Le client UI reçoit une erreur générique et ne peut pas mettre en surbrillance le champ en défaut. RFC 9457 encourage `detail` et `instance`.
- **Correctif suggéré** : Enrichir `Problem` avec `detail` indiquant le champ fautif (ex: `"invalid target currency: XXZ"` ou `"invalid amount at index 2"`).

#### MEDIUM-4 — `Money` n'offre aucune addition/somme contrôlée
- **Fichier:ligne** : `crates/core/src/money.rs:14-76`
- **Défaut** : Le type `Money` expose `amount()` mais pas `Add`/`Sum` ni de méthode `add_in_currency`. Les tests eux-mêmes additionnent `m1.amount() + m2.amount()` sans vérifier la devise.
- **Scénario d'échec concret** : Un développeur additionne par inadvertance des montants de devises différentes, obtenant un total faux sans erreur à la compilation ni à l'exécution.
- **Correctif suggéré** : Implémenter `Add<Money>` (qui erreur si devises différentes) ou ajouter une méthode `add(&self, other: &Money) -> Result<Money, DomainError>`.

---

### LOW

#### LOW-1 — `ConvertedTotalResponse` utilise `u32` pour `converted`/`excluded`
- **Fichier:ligne** : `crates/proto/src/lib.rs:386-388`, `crates/server/src/exchange.rs:189-190`
- **Défaut** : `core` compte en `usize`, le DTO de réponse en `u32`. Sur une liste de plus de 4 milliards d'éléments, la valeur est tronquée.
- **Scénario d'échec concret** : Très peu probable en pratique, mais le type du contrat ne reflète pas fidèlement le domaine.
- **Correctif suggéré** : Utiliser `u64` dans le DTO ou borner explicitement la taille de `amounts`.

#### LOW-2 — `Secret<T>` ne zeroize pas la mémoire
- **Fichier:ligne** : `crates/proto/src/lib.rs:27-64`
- **Défaut** : Le type masque bien `Debug`/`Display` et reste transparent sériel, mais il ne définit pas de `Drop` pour effacer la mémoire. Un `Clone` du `String` interne laisse le secret dans le tas.
- **Scénario d'échec concret** : Un dump mémoire, un core dump ou une fuite d'objet contient le mot de passe en clair alors que les logs eux sont propres.
- **Correctif suggéré** : Utiliser `secrecy::SecretString` ou documenter explicitement que le masquage se limite aux logs (ce qui répond à REQ-SEC-003 mais pas au durcissement mémoire).

---

### NIT

#### NIT-1 — Catégorie d'erreur inadéquate pour un taux non positif
- **Fichier:ligne** : `crates/core/src/exchange.rs:41-45`
- **Défaut** : `ExchangeRate::new` retourne `DomainError::InvalidMoney` pour `rate <= 0`. Ce n'est pas un montant.
- **Correctif suggéré** : Utiliser `DomainError::InvalidArgument` ou ajouter `DomainError::InvalidRate`.

#### NIT-2 — Duplication de logique dans `RateTable`
- **Fichier:ligne** : `crates/core/src/exchange.rs:132-159`
- **Défaut** : `rate` et `dated_rate` dupliquent le test `base == quote` et la recherche linéaire exacte.
- **Correctif suggéré** : Extraire une méthode privée `find_rate` ou implémenter `dated_rate` via `rate`.

#### NIT-3 — `SubscriptionDto` exige `active` sans valeur par défaut
- **Fichier:ligne** : `crates/proto/src/lib.rs:264`
- **Défaut** : Le domaine définit `active = true` par défaut (`Subscription::new`), mais le DTO de désérialisation n'a pas `#[serde(default)]` sur `active`.
- **Scénario d'échec concret** : Un client qui omet `active` reçoit une erreur de parsing, contrairement au comportement du domaine.
- **Correctif suggéré** : Ajouter `#[serde(default)]` sur `active` pour aligner DTO et domaine.

---

## Verdict global

Le cœur de domaine est correct sur les chemins heureux : utilisation de `Decimal`, arrondi bancaire, signalisation explicite des agrégats partiels, et absence de `unwrap`/`expect`/`panic` en production. Cependant, il présente des lacunes sérieuses aux frontières et dans la modélisation :

1. **Validation des devises** (HIGH-1, MEDIUM-1) : la frontière est trop perméable ; les codes hors référentiel et en minuscules passent.
2. **Contrat API** (HIGH-2) : les routes ne respectent pas le versionnement imposé.
3. **Modèle temporel** (HIGH-3) : `NaiveDate` est insuffisant pour les exigences d'échéance incluant l'heure locale.
4. **Modèle d'abonnement** (HIGH-4, HIGH-5) : incomplet par rapport à Wallos et aux exigences futures, et sans `owner_id` contrairement à §9.
5. **Robustesse** (MEDIUM-2, MEDIUM-3, MEDIUM-4) : corruption des taux ignorée, erreurs 422 trop génériques, opérations monétaires non contrôlées.

**Avant de considérer ce vertical comme terminé**, il faut durcir la validation des devises, versionner les routes, étendre le modèle `Subscription`, et renforcer les tests sur les cas limites (code devise invalide, minuscule, unsupported, taux corrompus). Les tests actuels couvrent les critères d'acceptation nominaux mais laissent des angles morts exploitables par `cargo-mutants`.
