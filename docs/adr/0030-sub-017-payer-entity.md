# ADR 0030 — Payeur : entité nominative légère + refus de suppression si référencé (oracle legacy)

- **Statut** : accepté (2026-08-04)
- **Contexte** : REQ-SUB-017 (« rattachement à un payeur »), exigence `oracle: legacy`, criticality
  medium, layer `[core, api, ui]`, dépend de REQ-SUB-001. Première exigence à capturer un oracle legacy.

## Problème

L'exigence permet de rattacher un abonnement à un **payeur** pour répartir les dépenses d'un foyer.
Deux points à trancher : le **modèle** du payeur (OQ-002 avait décidé « membre du foyer avec accès »,
ce qui impliquait le multi-utilisateur), et le **comportement de suppression** d'un payeur référencé
(« refusée ou réaffectés selon le comportement capturé sur l'application d'origine »).

Une hypothèse d'ADR 0016 supposait que Wallos 5.4.2 n'avait pas de notion de payeur. **Vérification sur
l'image épinglée : c'est faux.** Wallos a une table `household` (`id, name, email, user_id`) — des
**membres nominatifs** (pas de login par membre) — et `subscriptions.payer_user_id` (FK `household.id`).

## Décision

### Modèle : étiquette nominative légère (OQ-010)

Un payeur est une **entité nominative** possédée par le foyer : table `payers (id uuid, household_id,
name)`, `subscriptions.payer_id` la référence (rattachement facultatif). **Aucun login/compte par
payeur** — parité fidèle avec `household` de Wallos. Cela révise OQ-002 (le multi-membre avec accès
reste hors périmètre tant qu'aucune exigence ne l'impose). Pas d'unicité de nom (comme Wallos, et comme
les moyens de paiement). CRUD calqué sur `categories`/`payment_methods` (idempotent, isolé §9).

### Suppression d'un payeur référencé : REFUS (oracle gelé)

Comportement capturé sur Wallos 5.4.2 (`endpoints/household/household.php`, `handleDeleteMember`) et
**gelé** dans `e2e/fixtures/oracles/REQ-SUB-017-payer.json` : Wallos compte les abonnements référents
(`SELECT COUNT(*) FROM subscriptions WHERE payer_user_id = :id`) et, si non nul, renvoie `success:false`
/ `household_in_use` **sans supprimer** — jamais de réaffectation ni de cascade. subtrack mappe ce refus
sur **HTTP 409**. Un payeur non référencé se supprime (204). (Wallos protège aussi le membre `id=1` ;
subtrack n'auto-crée aucun payeur par défaut, donc sans objet.)

## Traçabilité de l'oracle

Conformément au protocole §8.1 tel que réellement pratiqué dans le dépôt : l'oracle est **capturé
manuellement** depuis l'image épinglée (inspection du schéma SQLite + du PHP) et **gelé** en fixture
JSON avec `_source` (digest + fonction PHP). La valeur de référence (refus → 409) est **asserée au
niveau intégration** (`referenced_payer_cannot_be_deleted`) et un scénario e2e `@design` vérifie l'UI
subtrack (le comptage par foyer, §9, n'étant pas rejouable sur Wallos mono-foyer — même traitement que
REQ-CAT-003). Wallos n'est **pas** exécuté en CI.

## Conséquences

- Nouvelles opérations `createPayer`/`listPayers`/`renamePayer`/`deletePayer` (couverture API + authz
  100 %). Le filtre `GET /subscriptions?payer=` (préexistant) reflète le rattachement (critère #1).
- **Répartition par payeur** (l'affichage statistique du critère #1) relève de **REQ-STA-004**, livré
  dans la foulée (dépendance inversée : STA-004 → SUB-017, OQ-010).
- L'e-mail des membres Wallos n'est pas repris (hors périmètre). Le rattachement d'un abonnement à un
  payeur via le **formulaire** d'abonnement (sélecteur) pourra être ajouté avec STA-004 ; l'API le
  supporte déjà (`CreateSubscriptionRequest.payer`).
