# ADR 0004 — Contrat OpenAPI minimal avant les premières opérations

## Contexte

`api/openapi.json` est un artefact attendu (R8). Avant l'implémentation des premières opérations, le document doit être valide mais vide, et non absent. Cela permet au drift gate de fonctionner dès la Phase 3.

## Décision

Générer un `OpenApi` utoipa minimal avec `info`, `servers`, et des tableaux vides. L'artefact est committé et comparé par `cargo xtask openapi --check`.

## Conséquences

- Le document est versionné dès maintenant.
- L'ajout d'une opération sans régénération ou avec drift déclenchera une erreur CI.

## Liens

- AGENTS.md §0 (R8), §6.
