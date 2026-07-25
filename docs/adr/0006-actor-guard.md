# ADR 0006 — Garde-fou d'isolation par `Actor`

## Contexte

AGENTS.md §9 impose que les repositories requièrent un contexte d'appelant (`Actor`) pour empêcher les requêtes SQL sans clause de propriétaire. Ce type doit être défini dans `core` avant toute implémentation `storage`.

## Décision

Introduire `core::actor::Actor` comme type opaque portant un identifiant d'utilisateur. Les traits de repository dans `core` prennent `&Actor` en premier argument. `storage` implémente ces traits avec `sqlx` et la clause `owner_id = $actor_id`.

## Conséquences

- Aucune requête repository ne peut omettre le contexte d'appelant : le type le rend impossible à compiler.
- 3 tests d'autorisation par opération (owner 2xx, other 404, anonymous 401) sont rattachés à REQ-SEC-001.

## Liens

- AGENTS.md §9.
- REQ-SEC-001.
