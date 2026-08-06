-- REQ-SYN-005 — Journal des conflits. Résolution « dernière écriture gagnante » (last-write-wins)
-- arbitrée par l'horodatage serveur : lorsqu'une écriture fondée sur une version PÉRIMÉE en écrase une
-- autre (concurrence optimiste), la version perdue est CONSERVÉE ici pour consultation par
-- l'utilisateur. De même, une modification écartée par une suppression concurrente (« la suppression
-- l'emporte ») y est journalisée. Isolation par foyer (§9). `recorded_at` fourni par le serveur.

create table conflict_journal (
    id           uuid        primary key default gen_random_uuid(),
    household_id uuid        not null references households (id),
    entity_type  text        not null,
    entity_id    uuid        not null,
    -- Version perdue (JSON de l'entité), et motif de la perte (`overwritten` / `deleted_remotely`).
    lost_payload text        not null,
    reason       text        not null,
    recorded_at  timestamptz not null default now()
);

-- Lecture par foyer, de la plus récente à la plus ancienne ; purge par `recorded_at`.
create index conflict_journal_household_recorded_idx
    on conflict_journal (household_id, recorded_at desc);
