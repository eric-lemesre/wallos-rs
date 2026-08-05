-- REQ-SYN-002 — Pierres tombales. Trace des SUPPRESSIONS d'entités possédées (catégories, moyens de
-- paiement, payeurs) pour qu'un appareil hors ligne applique la suppression au lieu de réintroduire une
-- entité qu'il croit vivante. Isolation par foyer (§9/SEC-001). `deleted_at` est fourni par le SERVEUR
-- (REQ-SYN-001, jamais l'horloge du client). Rétention par défaut 30 j, purge côté serveur (ADR 0013) :
-- au-delà, un appareil au curseur `since` périmé est contraint à une resynchronisation complète.
-- Une unique pierre tombale par (foyer, type, entité) : recréer puis resupprimer la même entité
-- rafraîchit `deleted_at` (upsert côté application).

create table tombstones (
    id           uuid        primary key default gen_random_uuid(),
    household_id uuid        not null references households (id),
    entity_type  text        not null,
    entity_id    uuid        not null,
    deleted_at   timestamptz not null default now(),
    unique (household_id, entity_type, entity_id)
);

-- Lecture par foyer, ordonnée par date de suppression (curseur `since`) ; purge par `deleted_at`.
create index tombstones_household_deleted_idx on tombstones (household_id, deleted_at);
