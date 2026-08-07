-- Livraisons de notification en échec (REQ-NOT-007) : réessai à intervalle croissant, borné,
-- puis abandon VISIBLE. Seuls les échecs sont journalisés (le succès immédiat est le cas nominal,
-- sans bruit) ; un réessai réussi supprime la ligne, la borne atteinte la passe en 'abandoned'.
create table notification_deliveries (
    id uuid primary key default gen_random_uuid(),
    household_id uuid not null references households (id),
    channel_id uuid not null references notification_channels (id) on delete cascade,
    as_of date not null,
    -- Charge utile du lot raté (ReminderNotification sérialisée) : le réessai renvoie le même lot.
    payload jsonb not null,
    attempts integer not null default 1,
    -- Prochaine tentative ('pending' uniquement ; null quand 'abandoned').
    next_attempt_at timestamptz,
    status text not null default 'pending' check (status in ('pending', 'abandoned')),
    -- Code de diagnostic redacté du dernier échec (jamais l'erreur brute).
    last_code text not null,
    updated_at timestamptz not null default now(),
    -- Un seul suivi par (canal, lot du jour) : l'échec du même lot ne crée pas de doublon.
    unique (channel_id, as_of)
);

create index notification_deliveries_household_idx
    on notification_deliveries (household_id, status);
create index notification_deliveries_retry_idx
    on notification_deliveries (status, next_attempt_at);
