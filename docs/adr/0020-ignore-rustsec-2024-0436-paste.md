# ADR 0020 — Ignorer des advisories `cargo-audit` non applicables (`paste`, `rsa`)

## Contexte

La porte CI n°15 (AGENTS.md §10) exécute **`cargo-deny`** (analyse le **graphe compilé**, respecte
features/targets) **et** `cargo-audit` (scanne **tout `Cargo.lock`**, de façon conservatrice). Deux
advisories font échouer `cargo audit` alors qu'ils ne concernent **aucun code réellement compilé** de
notre serveur PostgreSQL :

- **RUSTSEC-2024-0436 — `paste` 1.0.15 « unmaintained »**. Proc-macro **build-time** (jamais dans le
  binaire), tirée uniquement par `utoipa-axum` (dernière version publiée ; aucune montée ne la retire).
  Pas une vulnérabilité. `cargo deny check` la voit (paste EST compilée) → ignorée dans `deny.toml`.
- **RUSTSEC-2023-0071 — `rsa` 0.9.10 « Marvin Attack » (timing sidechannel, medium, sans correctif)**.
  `rsa` n'est tirée que par **`sqlx-mysql`** — le driver **MySQL**, que nous **n'utilisons pas**
  (serveur **PostgreSQL** uniquement). `cargo tree -i rsa` : **absente du graphe compilé** (aucun
  target/feature) ; `cargo deny check` (graphe) **passe** sans l'ignorer. Elle n'apparaît que dans
  `Cargo.lock` (Cargo y inscrit les drivers optionnels de `sqlx-macros-core` même non compilés), et
  seul `cargo audit` (lock) la signale. **Aucune opération RSA ne s'exécute** dans notre binaire.

Vérifié : `sqlx` est déclaré `features = [… "postgres" …]` sans `mysql` ; `default-features = false`
ne retire pas ces entrées du lock (structure de `sqlx-macros-core`) — donc l'élimination par features
est impossible sans abandonner `sqlx`.

## Décision

**Ignorer explicitement ces deux advisories**, chacun là où il est effectivement signalé :

- `deny.toml` → `[advisories].ignore` : `RUSTSEC-2024-0436` (paste, compilée). *(rsa n'y est pas :
  `cargo deny` ne la voit pas, le graphe étant postgres-only.)*
- Étape `audit` du workflow → `cargo audit --ignore RUSTSEC-2024-0436 --ignore RUSTSEC-2023-0071`.

Les ignores sont **strictement bornés à ces identifiants** : tout autre advisory — ou une vraie
vulnérabilité **dans le graphe compilé** (que `cargo deny` verrait) — continue de faire échouer la porte.

## Conséquences

- La porte 15 redevient un signal utile : `cargo deny` garde une couverture stricte du **graphe réel** ;
  `cargo audit` cesse d'échouer sur des deps **non compilées**.
- **Revue** : retirer `RUSTSEC-2024-0436` dès qu'une version de `utoipa-axum` sans `paste` paraît ;
  retirer `RUSTSEC-2023-0071` si `sqlx` cesse d'inscrire `sqlx-mysql`/`rsa` dans le lock, ou si un
  correctif `rsa` paraît.
- Aucune dépendance ajoutée ni retirée ; décision de **politique de sécurité**, pas de code.

## Liens

- AGENTS.md §0 (R6), §10 (porte 15) ; ADR 0002 (justification des dépendances), 0009 (client OpenAPI),
  0010 (PostgreSQL serveur). `deny.toml` (l.8-13), `.github/workflows/ci.yml` (étape `audit`).
- Advisories : https://rustsec.org/advisories/RUSTSEC-2024-0436 ,
  https://rustsec.org/advisories/RUSTSEC-2023-0071

## Statut

accepted
