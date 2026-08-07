-- Journal des envois de test de canal (REQ-NOT-006, revue F1) : limitation de taux par foyer.
-- Persistant (fenêtre glissante correcte même multi-instance, même après redémarrage).
create table notification_test_attempts (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    attempted_at timestamptz not null default now()
);

create index notification_test_attempts_household_idx
    on notification_test_attempts (household_id, attempted_at);
