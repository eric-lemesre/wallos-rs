# ADR 0002 — Justification rétroactive des dépendances structurantes

## Contexte

AGENTS.md §6 impose un ADR pour toute dépendance nouvelle. Le dépôt contient déjà ~40 dépendances déclarées avant la mise en place de la gouvernance. Avant d'ajouter la moindre dépendance supplémentaire, les dépendances structurantes déjà présentes sont justifiées par cet ADR collectif.

## Décision

Les dépendances suivantes, déjà déclarées dans le workspace, sont acceptées comme fondations du projet :

- **Async / runtime** : `tokio`, `tokio-util`, `tokio-cron-scheduler` — runtime async et ordonnanceur de tâches cron.
- **Web / API** : `axum`, `tower`, `tower-http`, `hyper` — serveur HTTP et middlewares.
- **OpenAPI** : `utoipa`, `utoipa-axum` — source de vérité code-first du contrat API.
- **Serialization** : `serde`, `serde_json` — types partagés.
- **Time** : `chrono`, `chrono-tz` — calculs de dates, échéances, fuseaux horaires.
- **Money** : `rust_decimal` — représentation exacte des montants (R4).
- **IDs** : `uuid` — identifiants internes.
- **DB** : `sqlx` — accès SQL asynchrone avec migrations.
- **Auth / security** : `argon2`, `password-auth`, `rand`, `secrecy` — hachage, mots de passe, secrets.
- **Errors** : `thiserror`, `anyhow` — erreurs typées et contexte.
- **Tracing** : `tracing`, `tracing-subscriber` — observabilité.
- **Config** : `config`, `dotenvy` — configuration.
- **HTTP client** : `reqwest` — client SDK HTTP.
- **Testing** : `tempfile`, `proptest`, `insta` — tests, propriétés, snapshots.
- **CLI / tooling** : `clap`, `xshell` — outillage xtask.
- **Proc-macro** : `proc-macro2`, `quote`, `syn` — macro `#[requirement]`.
- **YAML** : `yaml-rust2` — parsing des exigences.
- **Regex / walkdir** : `regex`, `walkdir` — lint-money.

Toute nouvelle dépendance à l'avenir fera l'objet d'un ADR dédié.

## Conséquences

- Le répertoire `docs/adr/` est créé et les dépendances existantes sont documentées.
- `R6` est désormais satisfait pour le passif ; les ajouts futurs seront soumis à ADR individuel.

## Liens

- AGENTS.md §0 (R6).
- Cargo.toml workspace.
