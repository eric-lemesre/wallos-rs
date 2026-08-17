# AGENTS.md — `subtrack`

> Contrat de travail des agents IA sur ce dépôt.
> Ce fichier fait autorité. En cas de conflit avec une consigne de chat, **ce fichier gagne**.
> Toute modification de ce fichier exige une entrée dans `docs/adr/`.

---

## 0. Règles non négociables

| # | Règle | Vérification |
|---|-------|--------------|
| R0 | Les commits créés par un agent IA **ne mentionnent jamais** `Co-authored-by` ni tout autre mécanisme d'attribution tierce. L'auteur d'un commit est l'humain responsable du dépôt. | revue + `git log` |
| R1 | Toute ligne de code de production est rattachée à **au moins une exigence** (`REQ-*`) | `cargo xtask trace` |
| R2 | Aucune exigence `status: accepted` sans **implémentation ET test** | `cargo xtask trace` |
| R3 | **Interdiction absolue** d'abaisser un seuil de couverture, de supprimer un test, de le marquer `#[ignore]` / `test.skip` pour faire passer la CI | revue + CI |
| R4 | Les montants monétaires utilisent `rust_decimal::Decimal`. `f32`/`f64` interdits dans `core` et `domain` | `cargo xtask lint-money` |
| R5 | `unwrap()`, `expect()`, `panic!` interdits hors `#[cfg(test)]` et hors `main.rs` | `clippy -D clippy::unwrap_used` |
| R6 | Aucune dépendance nouvelle sans ADR (`docs/adr/NNNN-*.md`) | revue |
| R7 | Aucune dépendance de coquille native hors de `frontend/shells/` : le code d'interface partagé ignore la plateforme et passe par l'adaptateur (REQ-CLT-003) | revue + `cargo xtask trace` |
| R8 | Le schéma OpenAPI committé doit être identique à celui généré | CI drift gate |
| R9 | **Dépôt unique** : serveur, interface, coquilles et recettes de paquets vivent dans `wallos-rs`. Aucun dépôt satellite, aucun sous-module | revue + ADR 0056 |
| R10 | **Parité intégrale** : toute fonctionnalité de l'application d'origine est reproduite. Un écart n'est recevable que s'il est **technique** (PHP → Rust/Tauri) ou un **sur-ensemble strict** dont le comportement d'origine reste obtenable. « Mieux » n'est pas un motif ; « en plus » l'est | revue + `oracle-coverage` + ADR 0060 |
| R11 | Toute exigence de parité porte un **test e2e** — sans exception — et un **test unitaire** dès qu'une règle calculable est en jeu, dont les vecteurs viennent de l'oracle et non de notre raisonnement | `cargo xtask trace` + revue |

**Protocole de blocage** — si une règle rend une tâche impossible, l'agent **s'arrête**, ajoute une entrée
dans `spec/OPEN-QUESTIONS.md` au format ci-dessous, et rend la main. Il ne contourne jamais.

```md
## OQ-014 — Titre court
- **Bloque** : REQ-SUB-031
- **Contexte** : …
- **Options** : A) … B) …
- **Recommandation agent** : B, car …
- **Statut** : open
```

---

## 1. Architecture cible

```
subtrack/
├── crates/
│   ├── core/            # Domaine pur. ZÉRO I/O, zéro async, zéro dépendance réseau.
│   │                    # Récurrences, échéances, conversion de devises, agrégats statistiques.
│   ├── proto/           # Types partagés + schémas API (serde + utoipa)
│   ├── storage/         # sqlx, migrations, repositories (traits définis dans core)
│   ├── server/          # axum, auth, scheduler de notifications, endpoints de sync
│   ├── notifier/        # canaux : email, webhook, telegram, discord, gotify, pushover
│   └── req-macros/      # proc-macro #[requirement(...)] — validation des IDs à la compilation
├── frontend/            # Espaces de travail npm (déclarés à la racine du dépôt)
│   ├── ui/              # 100 % de l'interface. Expose App({ canal, apiBaseUrl }).
│   │                    # `src/ds/` = design system (feuille unique + primitives + stories)
│   ├── api-client/      # Contrat généré depuis api/openapi.json, en paquet (ADR 0057)
│   └── shells/          # Coquilles. R7 : aucune dépendance de coquille hors d'ici.
│       ├── web/         # Vite + build statique servi par `server` (REQ-OPS-003)
│       ├── desktop/     # Coquille bureau — Linux, macOS, Windows (REQ-CLT-001)
│       └── mobile/      # Coquille mobile — Android, iOS (REQ-CLT-002)
├── packaging/           # Recettes serveur ET clients : conteneur, deb, rpm, archives,
│                        # dépôt signé (REQ-OPS-007/010/011/012, REQ-CLT-007)
├── e2e/
│   ├── specs/           # Scénarios AGNOSTIQUES de l'implémentation
│   ├── drivers/         # LegacyDriver (app d'origine) | TargetDriver (subtrack)
│   └── fixtures/        # Jeux de données extraits de l'app d'origine (oracles)
├── spec/
│   ├── requirements/    # Une exigence = un bloc YAML
│   ├── requirements.lock.yaml   # Index compilé (généré, committé)
│   └── OPEN-QUESTIONS.md
├── api/openapi.json     # Artefact généré, committé, verrouillé par la CI
├── docs/
│   ├── adr/
│   └── traceability.md  # Matrice générée
└── xtask/               # Outillage : trace, coverage, drift, lint-money
```

**Règle de dépendance** : `core` ne dépend de rien du projet. `storage`, `server`, `notifier`
dépendent de `core` et jamais l'inverse. Toute inversion est un échec d'architecture.

---

## 2. Cycle de vie d'une exigence

```
SPEC ──▶ CONTRAT (OpenAPI) ──▶ TESTS (rouges) ──▶ CODE ──▶ TRAÇABILITÉ ──▶ PORTES CI
  ▲                                                                            │
  └────────────────────────── retour si porte rouge ◀─────────────────────────┘
```

L'agent **n'écrit jamais de code avant d'avoir écrit le test qui échoue**. Un commit contenant
du code de production sans test associé dans le même commit est rejeté.

---

## 3. Format d'exigence

Fichier : `spec/requirements/<domaine>.md`. Un bloc YAML par exigence.

```yaml
---
id: REQ-SUB-012
title: Calcul de la prochaine échéance pour un cycle mensuel
domain: subscriptions
status: accepted          # draft | accepted | implemented | verified | deprecated
criticality: high         # high | medium | low
layer: [core, api, ui]    # où l'exigence doit être implémentée
e2e: required             # required | optional | n-a
oracle: legacy            # legacy | design  (legacy = comportement capturé sur l'app d'origine)
rationale: >
  L'utilisateur doit voir la date exacte du prochain prélèvement pour anticiper sa trésorerie.
acceptance:
  - given: un abonnement mensuel démarré le 31 janvier
    when: on calcule la prochaine échéance
    then: la date retournée est le 28 ou 29 février selon l'année
  - given: un abonnement mensuel dont l'échéance tombe un jour de changement d'heure
    when: on calcule la prochaine échéance
    then: l'heure locale de facturation est préservée
depends_on: [REQ-SUB-003]
---
```

**Conventions d'ID** : `REQ-<DOMAINE>-<NNN>`, jamais réutilisé, jamais renuméroté.
Domaines : `SUB` (abonnements), `CAT` (catégories), `CUR` (devises), `STA` (statistiques),
`NOT` (notifications), `AUT` (authentification), `SYN` (synchronisation), `I18N`, `SEC`,
`OPS` (exploitation et déploiement), `CLT` (clients web, bureau et mobile).

Une exigence `deprecated` conserve ses annotations jusqu'à suppression effective du code.

---

## 4. Annotations de traçabilité

### 4.1 Rust — vérifiée à la compilation

```rust
/// Calcule la prochaine échéance à partir du cycle de facturation.
#[requirement(REQ-SUB-012)]
pub fn next_due_date(start: NaiveDate, cycle: BillingCycle, tz: Tz) -> Result<NaiveDate, DomainError> {
    // …
}
```

La macro `#[requirement(...)]` (crate `req-macros`) lit `spec/requirements.lock.yaml` au moment
de la compilation et **échoue le build** si l'ID n'existe pas ou est `deprecated`.
C'est le mécanisme central : une annotation fantôme est impossible à committer.

### 4.2 Tests Rust

```rust
#[test]
#[verifies(REQ-SUB-012, case = "fin de mois")]
fn next_due_date_clamps_to_last_day_of_february() { /* … */ }
```

### 4.3 OpenAPI

```rust
#[utoipa::path(
    get, path = "/api/v1/subscriptions/{id}/next-due",
    operation_id = "getNextDue",
    extensions(("x-requirements" = json!(["REQ-SUB-012"]))),
)]
```

### 4.4 Frontend

```ts
/** @implements REQ-SUB-012 */
export function formatNextDue(sub: Subscription, locale: string): string { /* … */ }
```

### 4.5 Playwright

```ts
test('affiche la prochaine échéance en fin de mois',
  { tag: ['@REQ-SUB-012', '@oracle-legacy'] },
  async ({ app }) => { /* … */ });
```

---

## 5. Matrice de traçabilité — `cargo xtask trace`

Produit `docs/traceability.md` :

| REQ | Statut | Impl. | Tests unit. | API | UI | E2E | Verdict |
|-----|--------|-------|-------------|-----|----|-----|---------|
| REQ-SUB-012 | accepted | `core/due.rs:41` | 6 | `getNextDue` | `formatNextDue` | 2 | ✅ |

Codes d'échec (tous bloquants) :

| Code | Signification |
|------|---------------|
| `TRC-01` | Exigence `accepted` sans implémentation |
| `TRC-02` | Exigence `accepted` sans test unitaire |
| `TRC-03` | Exigence `e2e: required` sans scénario Playwright |
| `TRC-04` | Annotation référençant un ID inexistant |
| `TRC-05` | Exigence `layer: [api]` sans opération OpenAPI correspondante |
| `TRC-06` | Fichier de production sans aucune annotation (code orphelin) |
| `TRC-07` | `requirements.lock.yaml` désynchronisé de `spec/requirements/` |
| `ORC-01` | Exigence `oracle: legacy` sans oracle gelé ni dérogation datée (`oracle-coverage`, ADR 0059) |
| `ORC-02` | Oracle gelé sans niveau de preuve `_evidence` déclaré |

---

## 6. API — OpenAPI et couverture 100 %

**Source de vérité** : le code Rust annoté `utoipa` (*code-first*). `api/openapi.json` est un
artefact **généré et committé**. La CI régénère et compare : toute divergence échoue (R8).

Chaîne :

```
crates/proto (utoipa)  ──▶  api/openapi.json  ──▶  frontend/api-client/ (openapi-typescript)
                                    │
                                    └────────────▶  tests de conformité (schemathesis)
```

Exigences :
- Chaque opération porte un `operation_id` en camelCase, stable, et `x-requirements` non vide.
- Chaque réponse d'erreur suit RFC 9457 (`application/problem+json`), schéma unique `Problem`.
- Versionnement par préfixe `/api/v1`. Toute rupture de contrat exige un ADR.
- **Couverture d'API** : `cargo xtask api-coverage` vérifie que 100 % des `operation_id`
  sont exercés par au moins un test d'intégration `crates/server/tests/`. Seuil : **100 %, sans exception**.

**Couverture de code back-end** (`cargo llvm-cov`) :

| Cible | Lignes | Branches |
|-------|--------|----------|
| `core` | 100 % | 100 % |
| `storage`, `server`, `notifier`, `client` | 100 % | 95 % |

Exclusions : **uniquement** via `xtask/coverage-exclusions.toml`, chaque entrée portant une
justification écrite et un `REQ` ou `OQ` de référence. Aucun `#[coverage(off)]` en ligne.

> ⚠️ 100 % de couverture de lignes ne prouve rien sur la correction. `cargo-mutants` est exécuté
> **sur `core` uniquement**, en nightly CI, seuil de survivants toléré : 0. C'est cette porte,
> pas la couverture, qui garantit que les tests générés par l'IA sont réellement discriminants.

---

## 7. Frontend — UI partagée sur trois modalités

**Principe** (ADR 0057, repris d'`ergonomia`) : **une** implémentation de l'interface, servie par
**trois coquilles de quelques lignes** — web, bureau, mobile (ADR 0055).

```
frontend/ui         ← 100 % de l'interface : écrans, formulaires, navigation, i18n, design system.
                      Expose UN composant racine : App({ canal, apiBaseUrl }).
frontend/shells/*   ← index.html + main.tsx + vite.config.ts (+ projet natif). Rien d'autre.
```

Règles :
- Une coquille **monte `App`, un point c'est tout**. Elle ne compose pas d'écrans, n'importe pas de
  composant à l'unité, et ne connaît aucun chemin `../../ui/src/…`.
- Budget de code spécifique à une coquille : **≤ 300 lignes**, plafond très large — la référence
  tient en douze. Au-delà de quelques dizaines, c'est que de la composition a fui hors de `ui`.
- La plateforme se réduit à **deux paramètres** : `canal` et `apiBaseUrl`. Toute capacité
  réellement native (magasin de secrets, notification système) s'introduit **au cas par cas**, pas
  par une abstraction préalable.
- Un composant **n'importe jamais de CSS** : il pose des `className` définies dans la feuille unique
  du design system, importée une fois par point d'entrée (un `import "*.css"` casserait `typecheck`).
- Responsive : un seul jeu de composants, points de rupture `sm/md/lg`. Pas de composants
  « mobile » dupliqués.
- Tout élément interactif porte un `data-testid` stable, dérivé de l'exigence quand c'est pertinent
  (`data-testid="sub-next-due"`). Les sélecteurs CSS/XPath sont interdits dans les tests.

### 7.1 Choix figés

| Sujet | Décision | État |
|-------|----------|------|
| Composants | React 19 + TypeScript `strict` | **en place** |
| Client API | `openapi-typescript` + `openapi-fetch`, **générés** depuis `api/openapi.json` | **en place** |
| Formulaires | `react-hook-form` + `zod` | **en place** |
| i18n | `i18next`, clés typées, aucune chaîne littérale en JSX (`REQ-I18N-*`) | **en place** |
| Espaces de travail | npm workspaces à la racine ; contrat d'API en paquet `@wallos/api-client` | **en place** |
| Racine unique | `App({ canal, apiBaseUrl })` exporté par `ui` | **en place** |
| Design system | feuille unique `ui/src/ds/wallos-ux.css` : jetons **relevés sur la cible figée**, socle d'éléments, thèmes clair et sombre. Porte : aucune couleur littérale hors de la feuille | **en place** — primitives à venir |
| Atelier | Storybook dans `ui` (`npm run storybook`) ; écrans réels rendus sur MSW, garde-fou `storybook/mockApi.test.tsx`. Hors du job `frontend` : workflow dédié, galerie en artefact | **en place** — un domaine livré comme patron |
| Charte d'interaction | référence UX **documentant l'existant**, lacunes déclarées | *après le design system* |

> **Le tableau distingue l'existant de la cible, et c'est délibéré.** Il a longtemps figé TanStack
> Query, TanStack Router et Zustand — **aucun des trois n'a jamais été installé**. Un contrat qui
> décrit ce qui n'existe pas cesse de guider. Le choix d'un gestionnaire d'état serveur, d'un
> routeur et d'un stockage local est **rouvert** : il sera tranché par un ADR au moment où une
> exigence le rendra nécessaire, pas par anticipation.

**Règle de non-duplication de types** : aucun type d'entité métier n'est écrit à la main en
TypeScript. Tout type provient de `components['schemas']` du client généré. Un `interface
Subscription` rédigé manuellement est une erreur CI (`npm run ts-types-drift`), pas un choix de style.
C'est la protection principale contre la dérive silencieuse entre back-end et front-end quand le
code est produit par un agent.

---

## 8. E2E Playwright — conformance contre l'application d'origine

### 8.1 Mécanique de l'oracle

Les scénarios sont écrits **une fois**, contre une interface abstraite, et exécutés contre
**deux cibles** :

```ts
// e2e/drivers/AppDriver.ts
export interface AppDriver {
  login(user: User): Promise<void>;
  createSubscription(input: SubscriptionInput): Promise<void>;
  readSubscriptionCard(name: string): Promise<SubscriptionCard>;
  readMonthlyTotal(): Promise<string>;
  // …
}
```

- `LegacyDriver` pilote l'application d'origine (conteneur Docker figé sur un tag précis).
- `TargetDriver` pilote `subtrack`.

**Protocole obligatoire** pour toute exigence `oracle: legacy` :

1. Capturer le comportement de référence sur la cible figée, et **geler** le résultat dans
   `e2e/fixtures/oracles/REQ-XXX-NNN.json` en déclarant son **niveau de preuve** (`_evidence`) :

   | `_evidence` | Comment | Ce que ça vaut |
   |-------------|---------|----------------|
   | `observed` | scénario rejoué contre l'application en marche (`LegacyDriver`) | le plus fort — c'est le comportement, pas notre lecture |
   | `executed` | code d'origine exécuté dans l'image (`docker exec … php -r`, requête SQLite) | fort pour une règle de calcul ; muet sur ce qui est rendu |
   | `read` | source ou schéma lus | plausible, **non vérifié** — à ne retenir qu'à défaut |

2. Écrire le scénario dans `e2e/specs/`, taggé `@legacy`, asserté contre les valeurs gelées.
3. L'exécuter contre notre application. Ce test rouge pilote l'implémentation.

Les exigences `oracle: design` (fonctionnalités nouvelles) sont taggées `@design` et n'ont pas
d'oracle.

> ⚠️ **État réel de la couverture** (audit du 2026-08-17, ADR 0059). Sur 33 exigences
> `oracle: legacy` : **1** `observed`, **8** `executed`, **10** `read`, **14 sans aucune capture**.
> Le premier `observed` est REQ-STA-001, confronté le 2026-08-17 au comportement rendu par
> l'application en marche (`e2e/legacy/parity-monthly-cost.spec.ts`). `LegacyDriver` couvre
> désormais l'authentification **et** la création d'abonnement ; il s'étendra exigence par exigence. La porte `oracle-coverage` rend le décompte permanent,
> et REQ-CLT-009 étend le relevé à l'interface, où lire le code ne suffit pas.

**Passerelle vers les tests unitaires** : les oracles extraits (dates d'échéance, totaux,
arrondis, conversions) sont réinjectés comme fixtures dans les tests de `core`. C'est le
mécanisme qui empêche l'IA de « réinventer » une règle métier de manière plausible mais fausse.

### 8.2 Stratégie par modalité — limites réelles de l'outillage

Un seul niveau depuis le retrait du natif (OQ-009, ADR 0054 — les niveaux L2 desktop
`tauri-driver` et L3 mobile Maestro sont retirés ; la modalité mobile est la PWA responsive,
couverte par L1 en émulation de viewport au besoin) :

| Niveau | Cible | Outil | Portée | Seuil |
|--------|-------|-------|--------|-------|
| L1 | Web (`shells/web`) | Playwright, projets `chromium` **et** `webkit` | Suite complète, les deux cibles legacy/app | **90 %** des exigences `e2e: required` |

Le projet `webkit` de Playwright est **obligatoire** : une suite qui ne tourne qu'en Chromium
donne une fausse assurance sur macOS et Linux, où le moteur de rendu réel n'est pas Chromium.

### 8.3 Définition de la couverture E2E — 90 %

La couverture E2E est **une couverture d'exigences, pas de lignes** :

```
couverture_e2e = (# exigences `e2e: required` avec ≥1 scénario vert) / (# exigences `e2e: required`)
```

Seuil : **≥ 90 %**, calculé par `cargo xtask trace --e2e`. Les 10 % tolérés doivent être
explicitement listés dans `spec/e2e-waivers.yaml` avec justification et date de revue.
Un waiver sans date d'expiration est une erreur CI.

Discipline anti-flakiness : `retries: 0` en local, `retries: 1` en CI, tout test ayant échoué
puis réussi est signalé et doit être corrigé sous 48 h. `waitForTimeout` est interdit.

---

## 9. Authentification et isolation des comptes

Le produit est **multi-utilisateur avec comptes serveur**. Cela fait de l'isolation des données
une préoccupation transversale, et c'est le risque n°1 d'un back-end généré par IA : un agent
écrit spontanément `SELECT * FROM subscriptions WHERE id = $1`, sans clause de propriétaire.
Le test passe, la faille est en production.

**Décisions figées**

| Sujet | Décision |
|-------|----------|
| Hachage | `argon2id`, paramètres OWASP, jamais configurables par l'utilisateur |
| Session web | jeton opaque en cookie `HttpOnly` / `SameSite=Lax` / `Secure`, rotation à chaque privilège élevé |
| API (intégrations) | jeton **par appareil** (Bearer), révocable individuellement — REQ-AUT-005 re-cadré, ADR 0028 ; conservation sûre à la charge du client |
| JWT | **interdit** pour les sessions (pas de révocation immédiate) |
| Portée | toute entité métier porte `owner_id` non nullable |

**Garde-fou structurel** — les repositories n'exposent aucune méthode sans contexte d'appelant :

```rust
// INTERDIT
fn find_subscription(id: SubscriptionId) -> Result<Subscription>;

// IMPOSÉ — le type rend l'oubli impossible à compiler
fn find_subscription(actor: &Actor, id: SubscriptionId) -> Result<Subscription>;
```

**Porte CI dédiée** — `cargo xtask authz-coverage` exige, pour **chaque** `operation_id`
d'OpenAPI, au minimum trois tests d'intégration :

1. accès autorisé par le propriétaire ⟶ `2xx`
2. accès par un utilisateur tiers authentifié ⟶ `404` (jamais `403`, qui divulgue l'existence)
3. accès non authentifié ⟶ `401`

Seuil : **100 %, sans waiver possible**. Une opération sans ses trois tests échoue la CI au même
titre qu'un test rouge. Ces tests sont rattachés à `REQ-SEC-001` (isolation des comptes) par
`#[verifies(...)]`.

---

## 10. Portes CI (ordre d'exécution)

```
1.  fmt + clippy -D warnings                     ⟶ bloquant
2.  cargo xtask trace                            ⟶ bloquant (TRC-01..07)
3.  cargo test --workspace                       ⟶ bloquant
4.  cargo llvm-cov (seuils §6)                   ⟶ bloquant
5.  cargo xtask openapi --check (drift)          ⟶ bloquant
6.  cargo xtask api-coverage (100 %)             ⟶ bloquant
7.  cargo xtask authz-coverage (100 %)           ⟶ bloquant
8.  schemathesis (conformité contrat)            ⟶ bloquant
9.  npm run ts-types-drift                          ⟶ bloquant
10. vitest + couverture frontend/ui (≥ 90 %)     ⟶ bloquant
11. e2e L1 TARGET=app (chromium + webkit)        ⟶ bloquant
12. (retirée — e2e L2 desktop, OQ-009/ADR 0054)  ⟶ sans objet
13. cargo xtask trace --e2e (≥ 90 %)             ⟶ bloquant
14. cargo-mutants sur core                       ⟶ nightly, bloquant sur main
15. cargo-deny + audit                           ⟶ bloquant
```

---

## 11. Definition of Done

Une tâche est terminée si et seulement si :

- [ ] L'exigence existe, est `accepted`, et `requirements.lock.yaml` est à jour
- [ ] Le code porte `#[requirement(...)]` ; les tests portent `#[verifies(...)]`
- [ ] L'opération OpenAPI existe, `x-requirements` renseigné, `api/openapi.json` régénéré
- [ ] Les trois tests d'autorisation de l'opération existent et passent (§9)
- [ ] Aucun type TypeScript métier écrit à la main (`ts-types-drift` vert)
- [ ] Le scénario E2E existe et passe sur `TARGET=legacy` puis `TARGET=app` (si `oracle: legacy`)
- [ ] Toutes les portes CI sont vertes, aucun seuil modifié
- [ ] `docs/traceability.md` régénéré et committé
- [ ] Le statut de l'exigence est passé à `verified`

---

## 12. Boucle de travail de l'agent

```
1. Lire spec/OPEN-QUESTIONS.md  → si une OQ bloque la tâche, s'arrêter.
2. Sélectionner l'exigence la plus prioritaire en statut `accepted` sans implémentation.
3. Écrire/compléter les critères d'acceptation si ambigus → sinon ouvrir une OQ.
4. Écrire le scénario E2E, l'exécuter contre `legacy`, enregistrer l'oracle.
5. Écrire les tests unitaires `core` à partir de l'oracle. Vérifier qu'ils sont ROUGES.
6. Implémenter `core`, puis `storage`, `server`, `ui`.
7. Régénérer OpenAPI + client TS.
8. Exécuter les portes 1 à 11 localement.
9. Commit atomique : un commit = une exigence.
   Format : `feat(sub): REQ-SUB-012 calcul d'échéance mensuelle`
10. Mettre à jour le statut de l'exigence.
```

**Un commit = une exigence.** Un commit couvrant plusieurs `REQ` est rejeté à la revue.

---

## 13. Commandes

```bash
cargo xtask trace                 # matrice + codes TRC
cargo xtask trace --e2e           # couverture d'exigences E2E
cargo xtask openapi               # régénère api/openapi.json
cargo xtask openapi --check       # drift gate
cargo xtask api-coverage          # 100 % des operation_id testés
cargo xtask authz-coverage        # 3 tests d'autorisation par operation_id
cargo xtask lint-money            # interdiction des flottants monétaires
cargo llvm-cov --workspace --branch --fail-under-lines 100
cargo mutants -p subtrack-core

npm run ts-types-drift            # aucun type métier écrit à la main côté TS
pnpm e2e --target=legacy          # exécute la suite contre l'app d'origine
pnpm e2e --target=app             # exécute la suite contre subtrack
pnpm e2e:record                   # gèle les oracles
```

---

## 14. Décisions et hypothèses

Les lignes ✅ sont **arbitrées** : un agent ne les remet jamais en cause. Les lignes ⬜ sont des
valeurs par défaut, révisables par ADR, mais également jamais modifiées à l'initiative d'un agent.

| # | Décision / hypothèse | Statut |
|---|----------------------|--------|
| H1 | UI partagée en React 19 + TypeScript, packagée dans `frontend/ui` | ✅ |
| H2 | OpenAPI *code-first* via `utoipa` / `utoipa-axum`, artefact committé | ✅ |
| H3 | Serveur PostgreSQL (le volet « SQLite client natif » est caduc — OQ-009) | ✅ |
| H4 | Multi-utilisateur avec comptes serveur, argon2id + sessions opaques | ✅ |
| H5 | Synchronisation LWW par enregistrement + tombstones + curseur `since` | ⬜ |
| H6 | Le scheduler de notifications vit côté serveur, jamais côté client | ⬜ |
| H7 | L'application d'origine est figée sur un tag Docker précis pour la durée du projet | ⬜ |
| H8 | Réécriture propre : aucun code, traduction ou asset repris de l'application d'origine | ⬜ |
