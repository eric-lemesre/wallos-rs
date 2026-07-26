# ADR 0017 — Dépendances runtime du frontend (React, Vite, vitest, formulaires, i18n)

## Contexte

`AGENTS.md` §7.1 fige déjà les choix d'architecture frontend (React 19 + TypeScript strict,
`react-hook-form` + `zod`, `i18next`, TanStack Query/Router, Zustand). L'ADR 0009 a introduit le
client API généré (`openapi-typescript` / `openapi-fetch`). Le premier vertical avec une couche
`ui` (REQ-AUT-001, formulaire d'inscription) nécessite d'introduire les dépendances npm de
rendu, de formulaire, de validation, d'i18n et de test. R6 impose un ADR pour toute dépendance
nouvelle.

## Décision

Introduire dans `frontend/ui` les dépendances suivantes, conformes aux choix figés de §7.1 :

- **`react`**, **`react-dom`** (19) — rendu, modalité unifiée web/desktop/mobile.
- **`vite`** (+ `@vitejs/plugin-react`) — dev server et build.
- **`vitest`** + **`@testing-library/react`** + **`@testing-library/jest-dom`** + **`jsdom`** —
  tests de composants (porte §10 n°10, couverture ≥ 90 %).
- **`react-hook-form`** + **`@hookform/resolvers`** + **`zod`** — formulaires validés, schéma unique
  client/serveur (§7.1).
- **`i18next`** + **`react-i18next`** — internationalisation ; aucune chaîne littérale en JSX
  (REQ-I18N-002).

**Différées** (introduites quand une exigence les requiert, pas avant) : `@tanstack/react-query`,
`@tanstack/react-router`, `zustand`. Un formulaire d'inscription unique n'a besoin ni de routage,
ni de cache serveur, ni d'état global ; les ajouter maintenant créerait des dépendances inutilisées.

## Conséquences

- Le client API généré reste la **seule** source des types d'entités (ADR 0009, porte
  `ts-types-drift`) ; les schémas `zod` valident les entrées de formulaire, ils ne redéclarent pas
  les types métier.
- `frontend/ui` devient un paquet React testable (vitest + jsdom), sans coquille native
  (`@tauri-apps/*` interdit hors `shells/`, R7).
- La coquille web (`frontend/shells/web`, Vite) et l'e2e Playwright arrivent dans un incrément
  distinct (PR e2e), avec leurs propres dépendances.
- Versions épinglées via le lockfile committé ; `node_modules/` ignoré.

## Liens

- AGENTS.md §0 (R6, R7), §7, §7.1, §10 (portes 9 et 10) ; ADR 0009 (client API).
- Exigences : REQ-AUT-001 (formulaire d'inscription), REQ-I18N-002 (pas de chaîne littérale).

## Statut

accepted
