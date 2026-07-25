# ADR 0008 — `xtask` dépend de `wallos-server` pour générer l'OpenAPI

## Contexte

`cargo xtask openapi` doit produire `api/openapi.json` à partir du contrat code-first défini dans
`crates/server` via `utoipa`. `xtask` étant un outillage séparé, il n'a pas accès au type
`wallos_server::ApiDoc` sans déclarer une dépendance.

## Décision

Ajouter `wallos-server` comme dépendance de `xtask`. La génération utilise
`wallos_server::ApiDoc::openapi().to_json()` et écrit `api/openapi.json`. La vérification `--check`
compare octet à octet (après normalisation JSON) avec le fichier committé.

## Conséquences

- `xtask` est un peu plus lourd à compiler, mais reste local au workspace.
- Aucune dépendance externe nouvelle n'est ajoutée (réutilisation interne).
- Toute opération ajoutée dans `server` est reflétée dans `api/openapi.json` dès la régénération.

## Liens

- AGENTS.md §0 (R6, R8), §6.
