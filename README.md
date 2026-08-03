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
- **Notifications multi-canaux** — email, webhook, Telegram, Discord, Gotify, Pushover *(en cours)*
- **Synchronisation** multi-appareils *(en cours)*
- **Multi-langue** — interface i18n (français / anglais), aucune chaîne d'affichage en dur
- **Auto-hébergement** — données chez vous, aucune dépendance à un service tiers
- **Coquille native desktop / mobile** via Tauri v2 *(planifié)*

---

## Architecture

Monorepo à deux versants : un workspace Cargo (backend + domaine) et un frontend React partagé entre coquilles web et natives.

```
wallos-rs/
├── crates/
│   ├── core/         # Domaine pur. ZÉRO I/O, zéro async, zéro réseau.
│   │                 # Récurrences, échéances, conversion de devises, agrégats statistiques.
│   ├── proto/        # Types partagés + schémas OpenAPI (serde + utoipa)
│   ├── storage/      # sqlx, migrations Postgres, repositories (isolation par foyer)
│   ├── server/       # axum : auth, handlers, scheduler de notifications
│   ├── notifier/     # canaux email / webhook / telegram / discord / gotify / pushover
│   ├── client/       # SDK HTTP Rust (réutilisé par le desktop)
│   ├── desktop/      # Tauri v2 — coquille native uniquement
│   └── req-macros/   # proc-macro #[requirement(...)] : validation des IDs à la compilation
├── frontend/
│   ├── ui/           # Composants + logique de vue PARTAGÉS (React, i18next, client openapi-fetch)
│   └── shells/web/   # Vite : build statique servi par `server`
├── e2e/
│   ├── specs/        # Scénarios AGNOSTIQUES de l'implémentation (Playwright)
│   └── drivers/      # LegacyDriver (Wallos d'origine) | TargetDriver (wallos-rs)
├── spec/             # Exigences (REQ-*), lock de traçabilité, questions ouvertes
├── xtask/            # Portes maison : trace, openapi, coverage, lint-money, lint-clock
└── docs/adr/         # Décisions d'architecture (ADR)
```

**Pile technique** — Rust 2024 / axum 0.8 / sqlx (Postgres) / utoipa · React + Vite + openapi-fetch + i18next · Playwright · Tauri v2.

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

```bash
cd frontend/ui
npm ci
npm run generate:api          # régénère le client typé depuis api/openapi.json
npm run dev                    # serveur Vite
```

> Toute modification de `proto` impose de régénérer le contrat :
> `cargo xtask openapi` puis `npm run generate:api` dans `frontend/ui`.

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

# Frontend
cd frontend/ui && npm test

# End-to-end (Playwright démarre le serveur Rust + Vite)
cd e2e && DATABASE_URL=postgres://postgres:postgres@localhost:5433/wallos npx playwright test
```

Les tests d'intégration provisionnent des bases éphémères via `#[sqlx::test]`. Les scénarios e2e s'exécutent au choix contre `TargetDriver` (wallos-rs) ou `LegacyDriver` (Wallos d'origine) pour comparaison d'oracle.

---

## Avancement

Le [badge d'exigences](#wallos-rs--suivi-dabonnements-personnels-réécrit-en-rust) en tête de page est **généré automatiquement** : il pointe (via un endpoint shields.io) vers `spec/trace-badge.json`, régénéré par `cargo xtask trace --write` et maintenu à jour par une porte de *drift* en CI. La ventilation détaillée, exigence par exigence, vit dans la **[matrice de traçabilité](spec/TRACEABILITY.md)** (elle aussi générée).

Instantané au dernier passage :

- **43** exigences `verified`
- **2** exigences `implemented`
- **28** exigences `draft`

Domaines les plus avancés : authentification et sessions, abonnements (CRUD, recherche/tri), devises et conversion, statistiques (coût mensuel, évolution sur douze mois). En cours / à venir : notifications multi-canaux, synchronisation multi-appareils, coquilles natives Tauri.

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
