-- REQ-NOT-001 — Rappels avant échéance.
-- Délai de rappel par compte (foyer) : nombre de jours avant l'échéance où le rappel est émis. Défaut 1
-- (parité Wallos notification_settings.days). Réglage d'account, jamais du client (comme la devise de
-- référence). Contrainte : entre 0 et 365 jours.
alter table households add column reminder_lead_days integer not null default 1;
alter table households
    add constraint households_reminder_lead_days_range check (reminder_lead_days between 0 and 365);

-- Journal des rappels ÉMIS : trace (foyer, abonnement, date d'échéance) pour lesquels un rappel a été
-- envoyé, afin qu'une ré-exécution du cron le même jour ne ré-émette pas (préparation de l'idempotence
-- REQ-NOT-002). `emitted_at` fourni par le serveur. Unicité sur (foyer, abonnement, date d'échéance).
create table reminder_log (
    id              uuid        primary key default gen_random_uuid(),
    household_id    uuid        not null references households (id),
    subscription_id uuid        not null references subscriptions (id) on delete cascade,
    due_date        date        not null,
    emitted_at      timestamptz not null default now(),
    unique (household_id, subscription_id, due_date)
);

create index reminder_log_household_idx on reminder_log (household_id, due_date);
