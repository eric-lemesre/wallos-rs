# ADR 0001 — Honnêteté des portes xtask

## Contexte

Les sous-commandes `cargo xtask trace`, `openapi`, `api-coverage`, `authz-coverage` et `lint-money` existaient sous forme de stubs qui affichaient un message "not yet implemented" et appelaient `std::process::exit(0)`. Cela violait l'esprit de R3 (AGENTS.md) : une porte CI qui ne vérifie rien mais signale le succès donne une fausse assurance de conformité.

## Décision

Remplacer les stubs par des implémentations minimales mais capables d'échouer avec un code de sortie non nul quand une règle est enfreinte, ou quand le périmètre attendu n'est pas couvert. Les commandes doivent être honnêtes avant d'être complètes.

## Conséquences

- `trace` parse `spec/requirements/*.md`, (re)génère `spec/requirements.lock.yaml`, et vérifie les codes TRC-01..07.
- `openapi` génère un document minimal valide quand aucune opération n'existe, et échoue sur drift.
- `api-coverage` et `authz-coverage` reportent un échec dès qu'une opération existe sans test.
- `lint-money` étend son scan à tous les crates de production (core, proto, storage, server, notifier, client, desktop), pas seulement `core`.
- Des tests d'intégration de `xtask` exercent chaque code d'échec.

## Liens

- AGENTS.md §0 (R3), §5, §6, §9, §10.
- Requirements : REQ-SEC-001 (fondation), REQ-AUT-001 (première opération couverte).

## Statut

accepted
