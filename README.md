# Wallos-rs — Suivi d'abonnements personnels, réécrit en Rust

> Réimplémentation **spec-driven** de [Wallos](https://github.com/ellite/Wallos), l'application web open-source et auto-hébergeable de suivi d'abonnements et de dépenses récurrentes.
>
> Objectif : la même valeur d'usage que l'original (PHP/SQLite), portée sur une pile **Rust + Postgres + React**, où **chaque ligne de code de production est rattachée à une exigence tracée** et vérifiée par des portes automatiques.

<p>
  <img alt="Rust" src="https://img.shields.io/badge/Rust-1.86-000?logo=rust">
  <img alt="Edition" src="https://img.shields.io/badge/edition-2024-000?logo=rust">
  <img alt="License" src="https://img.shields.io/badge/license-AGPL--3.0--or--later-blue">
  <img alt="Exigences" src="https://img.shields.io/endpoint?url=https://raw.githubusercontent.com/eric-lemesre/wallos-rs/main/spec/trace-badge.json">
</p>

---

## Sommaire

- [Pourquoi cette réécriture](#pourquoi-cette-réécriture)
- [Fonctionnalités](#fonctionnalités)
- [Architecture](#architecture)
- [Développement piloté par les exigences](#développement-piloté-par-les-exigences)
- [Prise en main](#prise-en-main)
- [Portes de qualité (`cargo xtask`)](#portes-de-qualité-cargo-xtask)
- [Tests](#tests)
- [Avancement](#avancement)
- [Contribuer](#contribuer)
- [Licence](#licence)

---

## Pourquoi cette réécriture

Wallos-rs n'est pas un simple portage. C'est un banc d'essai d'un flux de travail **piloté par les exigences** où :

- **Aucune ligne de production sans exigence.** Chaque fonction porte une annotation `#[requirement(REQ-…)]` validée à la compilation ; la matrice de traçabilité (`cargo xtask trace`) échoue si une exigence `accepted` n'a pas d'implémentation **et** de test.
- **L'original sert d'oracle.** Les scénarios end-to-end sont écrits de façon agnostique et rejouables sur deux pilotes : `LegacyDriver` (Wallos d'origine, image figée `bellamy/wallos:5.4.2`) et `TargetDriver` (wallos-rs). Le comportement observable doit coïncider.
- **Le schéma d'API est la source de vérité.** L'OpenAPI est généré depuis le code Rust (`utoipa`) et le client TypeScript du frontend en est dérivé ; une porte de *drift* interdit toute divergence.
- **La sécurité et l'argent sont non négociables.** Isolation stricte par foyer, montants en `Decimal` (jamais de flottant), aucun `unwrap`/`panic` en production — le tout vérifié par des portes.

Le contrat complet des règles se trouve dans [`AGENTS.md`](AGENTS.md).

---

## Fonctionnalités

Périmètre cible, à parité avec Wallos (les cases cochées sont **vérifiées** aujourd'hui, voir [Avancement](#avancement)) :

- **Suivi des abonnements** — montants, cycles de facturation, prochaines échéances, rappels
- **Recherche et tri** des abonnements (repli de diacritiques, tri par nom / montant / échéance)
- **Catégories personnalisables** de dépenses
- **Multi-devises** avec taux de change et conversion vers une devise de référence
- **Statistiques** — coût mensuel normalisé, évolution du coût sur douze mois glissants, répartitions
- **Authentification** — session par cookie ou jeton `Bearer`, jetons d'appareil, changement de mot de passe, limitation des tentatives
- **Isolation par foyer** — chaque foyer ne voit que ses propres données
- **Notifications multi-canaux** — email, webhook, Telegram, Discord, Gotify, Pushover
- **Synchronisation** multi-appareils
- **Multi-langue** — interface i18n (français / anglais), aucune chaîne d'affichage en dur
- **Auto-hébergement** — données chez vous, aucune dépendance à un service tiers

Le périmètre **fonctionnel** ci-dessus est vérifié. Deux chantiers sont **spécifiés mais pas encore
construits** — les exigences existent, le code non :

- **Déploiement** *(spécifié)* — image conteneur, paquets `.deb` / `.rpm`, archives autonomes,
  artefacts signés, sauvegarde et restauration vérifiées (domaine `OPS`). Aujourd'hui, l'installation
  passe encore par la compilation des sources ; voir [Prise en main](#prise-en-main).
- **Trois clients : web, bureau et mobile** *(spécifié)* — une interface unique, empaquetée par des
  coquilles natives derrière un adaptateur de plateforme (domaine `CLT`). C'est une **divergence
  assumée** avec l'application d'origine, qui n'a qu'un client web : depuis l'[ADR 0055](docs/adr/0055-native-clients-back-in-scope.md),
  la parité régit le *comportement métier*, plus le *périmètre des modalités*.

---

## Architecture

**Dépôt unique** (règle R9, [ADR 0056](docs/adr/0056-single-repository.md)) : serveur, interface, coquilles et recettes d'empaquetage vivent ici. Ce n'est pas une préférence de style — les portes de qualité du projet (traçabilité, dérive du contrat d'API, version commune client/serveur) ne fonctionnent que si le générateur et le généré sont commités ensemble.

Les entrées marquées *(à créer)* sont **spécifiées et localisées, pas encore construites**.

```
wallos-rs/
├── crates/
│   ├── core/         # Domaine pur. ZÉRO I/O, zéro async, zéro réseau.
│   │                 # Récurrences, échéances, conversion de devises, agrégats statistiques.
│   ├── proto/        # Types partagés + schémas OpenAPI (serde + utoipa)
│   ├── storage/      # sqlx, migrations Postgres, repositories (isolation par foyer)
│   ├── server/       # axum : auth, handlers, scheduler de notifications
│   ├── notifier/     # canaux email / webhook / telegram / discord / gotify / pushover
│   └── req-macros/   # proc-macro #[requirement(...)] : validation des IDs à la compilation
├── frontend/         # Espaces de travail npm (lock unique à la racine)
│   ├── api-client/   # Contrat TypeScript GÉNÉRÉ depuis api/openapi.json
│   ├── ui/           # 100 % de l'interface. Expose App({ canal, apiBaseUrl }).
│   │                 # Ignore la plateforme : seul le canal reçu la distingue (REQ-CLT-003).
│   └── shells/       # R7 : aucune dépendance de coquille hors d'ici
│       ├── web/      # index.html + un main.tsx de 15 lignes + vite.config.ts
│       ├── desktop/  # Linux / macOS / Windows          (à créer — REQ-CLT-001)
│       └── mobile/   # Android / iOS                    (à créer — REQ-CLT-002)
├── packaging/        # Conteneur, deb, rpm, archives, dépôt signé
│                     #                                   (à créer — REQ-OPS-007/010/011/012)
├── e2e/
│   ├── specs/        # Scénarios AGNOSTIQUES de l'implémentation (Playwright)
│   └── drivers/      # LegacyDriver (Wallos d'origine) | TargetDriver (wallos-rs)
├── spec/             # Exigences (REQ-*), lock de traçabilité, questions ouvertes
├── xtask/            # Portes maison : trace, openapi, coverage, lint-money, lint-clock
└── docs/adr/         # Décisions d'architecture (ADR)
```

**Pile technique** — Rust 2024 / axum 0.8 / sqlx (Postgres) / utoipa · React + Vite + openapi-fetch + i18next · Playwright. La technologie des coquilles natives n'est **pas arrêtée** : Tauri v2 est le candidat, l'engagement relèvera d'un ADR d'implémentation.

Principes de couches : `core` est un domaine **pur et sans horloge** (porte `lint-clock`) ; `storage` prend un `&Actor` pour l'isolation ; `server` ne fait qu'orchestrer ; le frontend consomme **exclusivement** le client typé généré depuis l'OpenAPI.

---

## Développement piloté par les exigences

Les exigences vivent dans [`spec/requirements/`](spec/requirements/), regroupées par domaine, et leur statut fait autorité dans `spec/requirements.lock.yaml` :

| Préfixe | Domaine |
|---------|---------|
| `REQ-AUT` | Authentification et sessions |
| `REQ-SUB` | Abonnements |
| `REQ-CAT` | Catégories |
| `REQ-CUR` | Devises et taux de change |
| `REQ-STA` | Statistiques |
| `REQ-NOT` | Notifications |
| `REQ-SYN` | Synchronisation |
| `REQ-SEC` | Sécurité / isolation |
| `REQ-I18N` | Internationalisation |
| `REQ-OPS` | Exploitation |

Cycle de vie d'une exigence : `draft → accepted → implemented → verified`. Une exigence ne passe `accepted` **que** dans le commit qui l'implémente et la teste (sinon `trace` échoue), puis `verified` une fois la revue et les portes vertes.

Les points en suspens sont consignés dans [`spec/OPEN-QUESTIONS.md`](spec/OPEN-QUESTIONS.md) ; les choix structurants dans [`docs/adr/`](docs/adr/).

---

## Prise en main

> **Aujourd'hui, l'installation passe par la compilation des sources.** Image conteneur, paquets
> `.deb` / `.rpm` et archives autonomes sont **spécifiés** (domaine `OPS`) mais pas encore publiés :
> ce qui suit est un environnement de **développement**, pas une procédure de mise en production.

### Prérequis

- **Rust 1.86+** (toolchain épinglée par `rust-toolchain.toml`)
- **Docker** (pour Postgres de test / dév) et **Node.js** (frontend + e2e)

### Base de données de développement

```bash
# Postgres de test/dév sur le port 5433
docker run -d --name wallos-pg-test \
  -e POSTGRES_PASSWORD=postgres -e POSTGRES_DB=wallos \
  -p 5433:5432 postgres:16

export DATABASE_URL=postgres://postgres:postgres@localhost:5433/wallos
```

Après un redémarrage de la machine : `docker start wallos-pg-test`.

### Backend

```bash
cargo build
cargo run -p wallos-server    # démarre l'API axum
```

### Frontend

Les paquets front sont des **espaces de travail npm** : une seule installation à la racine les
couvre tous (`api-client`, `ui`, coquilles).

```bash
npm ci                        # à la racine du dépôt
npm run generate:api          # régénère le contrat typé (@wallos/api-client)
npm run dev:web               # serveur Vite de la coquille web
```

> Toute modification de `proto` impose de régénérer le contrat :
> `cargo xtask openapi` puis `npm run generate:api` à la racine du dépôt.

---

## Portes de qualité (`cargo xtask`)

Ces portes tournent en CI et doivent rester vertes :

| Commande | Vérifie |
|----------|---------|
| `cargo xtask trace` | Matrice de traçabilité exigences ↔ code ↔ tests |
| `cargo xtask openapi --check` | Le `api/openapi.json` committé est identique au généré (drift) |
| `cargo xtask api-coverage` | Chaque opération d'API est couverte |
| `cargo xtask authz-coverage` | Tests `authz_{owner,other,anon}_<op>` présents par opération |
| `cargo xtask lint-money` | Aucun `f32`/`f64` dans les montants |
| `cargo xtask lint-clock` | `core` n'accède jamais à l'horloge système |

Complétées par `cargo clippy` (dont `-D clippy::unwrap_used`) et le lint frontend.

---

## Tests

```bash
# Unitaires + intégration (Postgres requis)
DATABASE_URL=postgres://postgres:postgres@localhost:5433/wallos cargo test

# Frontend (depuis la racine)
npm test

# End-to-end (Playwright démarre le serveur Rust + Vite)
cd e2e && DATABASE_URL=postgres://postgres:postgres@localhost:5433/wallos npx playwright test
```

Les tests d'intégration provisionnent des bases éphémères via `#[sqlx::test]`. Les scénarios e2e s'exécutent au choix contre `TargetDriver` (wallos-rs) ou `LegacyDriver` (Wallos d'origine) pour comparaison d'oracle.

---

## Avancement

Le [badge d'exigences](#wallos-rs--suivi-dabonnements-personnels-réécrit-en-rust) en tête de page est **généré automatiquement** : il pointe (via un endpoint shields.io) vers `spec/trace-badge.json`, régénéré par `cargo xtask trace --write` et maintenu à jour par une porte de *drift* en CI. La ventilation détaillée, exigence par exigence, vit dans la **[matrice de traçabilité](spec/TRACEABILITY.md)** (elle aussi générée).

Le badge est la seule source à jour de ce décompte : il est régénéré à chaque passage de la porte, là
où un chiffre recopié dans ce README vieillirait en silence.

Le **périmètre fonctionnel** de parité est vérifié de bout en bout — abonnements, catégories, devises
et conversion, statistiques, authentification et sessions, notifications multi-canaux, synchronisation
multi-appareils, internationalisation. Restent en `draft` les deux chantiers annoncés plus haut :
**déploiement** (`OPS`) et **clients natifs** (`CLT`).

Le décompte de `verified` a **reculé** d'un cran le 2026-08-16, lorsque le retour des clients natifs
dans le périmètre a rendu de nouveau exigible la notification système de `REQ-NOT-008`. Un badge qui
recule est le comportement attendu : il suit le périmètre réel, il ne le flatte pas.

---

## Contribuer

1. Lire [`AGENTS.md`](AGENTS.md) — il fait autorité sur les règles non négociables (R0–R8).
2. Toute contribution de code cible une exigence `REQ-*` (en créer une en `draft` si besoin).
3. Créer une branche **avant** tout commit (jamais de commit direct sur `main`).
4. Faire passer les portes localement (`cargo xtask …`, `cargo test`, `cargo clippy`) avant d'ouvrir la PR.
5. Les décisions structurantes s'accompagnent d'un ADR dans `docs/adr/`.

---

## Licence

Distribué sous **GNU Affero General Public License v3.0 ou ultérieure** (AGPL-3.0-or-later) — voir [`LICENSE`](LICENSE).

Wallos-rs est une œuvre dérivée indépendante inspirée de [Wallos](https://github.com/ellite/Wallos) (licence GPLv3) de Ellite. Merci au projet original pour la conception fonctionnelle qui sert de référence à cette réécriture.
