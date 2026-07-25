# ADR 0009 — Client API TypeScript généré (openapi-typescript / openapi-fetch)

## Contexte

AGENTS.md §7.1 fige le client API du frontend : `openapi-typescript` + `openapi-fetch`,
**générés** depuis `api/openapi.json`, avec une règle de non-duplication de types
(§7, porte `ts-types-drift`, porte CI n°9). La Phase 3 du plan verrouille le contrat en
amont d'un frontend complet. Ces paquets npm sont de nouvelles dépendances : R6 impose un ADR.

## Décision

Introduire, dans `frontend/ui`, les dépendances npm suivantes :

- **`openapi-typescript`** (devDependency) — génère `src/api/schema.d.ts` à partir de
  `api/openapi.json`. Aucun type d'entité métier n'est écrit à la main (AGENTS.md §7).
- **`openapi-fetch`** (dependency) — client HTTP typé consommant les types générés.
- **`typescript`** (devDependency) — compilation `strict` exigée par AGENTS.md §7.1.

Deux portes npm sont ajoutées :

- `generate:api` — régénère `src/api/schema.d.ts` depuis `api/openapi.json`.
- `ts-types-drift` — régénère puis échoue (`git diff --exit-code`) si le fichier committé
  diverge du contrat régénéré. C'est la protection contre la dérive silencieuse back/front.

## Conséquences

- Le contrat OpenAPI (R8) devient la source unique des types front ; toute évolution du
  contrat force la régénération, sinon `ts-types-drift` échoue.
- Les dépendances restent cantonnées à `frontend/ui` ; aucun `@tauri-apps/*` n'est introduit
  (R7 respecté, ce module n'est pas une coquille).
- `node_modules/` est ignoré ; le lockfile est committé pour la reproductibilité.

## Liens

- AGENTS.md §0 (R6, R8), §7, §7.1, §10 (porte 9).
- Plan `.hermes/plan.md` Phase 3.
- Requirements : REQ-SEC-002 (schéma d'erreur `Problem` exposé au client), REQ-OPS-001.

## Statut

accepted
