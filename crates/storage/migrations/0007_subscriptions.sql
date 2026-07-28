-- REQ-SUB-002 / SUB-001 — abonnements. Entité POSSÉDÉE (isolation par foyer, §9). Montant en `numeric`
-- (jamais un flottant, R4). Le cycle est stocké décomposé (unité + intervalle). La prochaine échéance
-- n'est PAS stockée : elle est dérivée à la lecture (next_due, ADR 0022) pour éviter toute péremption.

create table subscriptions (
    id                uuid        primary key,
    household_id      uuid        not null references households (id),
    name              text        not null,
    amount            numeric     not null,
    currency          text        not null,
    cycle_unit        text        not null,
    cycle_interval    integer     not null,
    first_payment     date        not null,
    category_id       uuid,
    payment_method_id uuid,
    payer_id          uuid,
    logo              text,
    url               text,
    notes             text,
    active            boolean     not null default true,
    created_at        timestamptz not null default now()
);

create index subscriptions_household_idx on subscriptions (household_id);
