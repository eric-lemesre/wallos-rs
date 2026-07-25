# ADR 0003 — Portée de lint-money

## Contexte

`cargo xtask lint-money` ne scannait que `crates/core`. AGENTS.md §6 et R4 interdisent les flottants pour tous les montants monétaires, ce qui concerne aussi les couches exposant des montants (`proto`, `storage`, `server`, `notifier`, `client`, `desktop`).

## Décision

Étendre le scan à tous les crates de production. Les exclusions justifiées seront listées dans `xtask/coverage-exclusions.toml` avec leur raison et leur référence REQ/OQ.

## Conséquences

- `lint-money` reporte une violation si `f32`/`f64` apparaît dans n'importe quel crate de production.
- `xtask` lui-même n'est pas un crate de production métier ; il peut utiliser `f64` si nécessaire.

## Liens

- AGENTS.md §0 (R4), §6.
