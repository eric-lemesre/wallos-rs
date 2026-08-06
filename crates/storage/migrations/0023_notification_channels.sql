-- REQ-NOT-005 — Canaux de notification, entité POSSÉDÉE (isolation par foyer, §9/SEC-001).
-- Abstraction unique partagée par tous les canaux (webhook en premier, puis e-mail NOT-003 et
-- messageries NOT-004) : le TYPE est porté par `kind`, la configuration propre au type par `config`
-- (jsonb). Pour un webhook : `config = { "url": "https://…" }`. Un canal `enabled = false` n'émet
-- aucune requête sortante (NOT-004). Config d'account (comme le délai de rappel / la devise de
-- référence), jamais synchronisée côté client : pas de pierre tombale.
create table notification_channels (
    id           uuid        primary key default gen_random_uuid(),
    household_id uuid        not null references households (id),
    kind         text        not null,
    config       jsonb       not null,
    enabled      boolean     not null default true,
    created_at   timestamptz not null default now(),
    updated_at   timestamptz not null default now()
);

create index notification_channels_household_idx on notification_channels (household_id);
