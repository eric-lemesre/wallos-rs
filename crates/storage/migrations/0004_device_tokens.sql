-- REQ-AUT-005 — jetons d'appareil pour les coquilles natives (desktop/mobile).
-- Jeton opaque long, propre à un appareil, révocable individuellement (par `id`), jamais stocké en
-- clair : seule son empreinte SHA-256 (`token_hash`, ADR 0018) l'est. `last_seen_at` est fourni par
-- l'appelant (instant injecté, cohérent avec le principe de reproductibilité — cf. sessions).
-- Pas d'`expires_at` : contrairement à une session web, un appareil reste appairé jusqu'à révocation.

create table device_tokens (
    id           uuid        primary key,
    token_hash   bytea       not null unique,
    user_id      uuid        not null references users (id),
    household_id uuid        not null references households (id),
    label        text        not null,
    platform     text        not null,
    created_at   timestamptz not null default now(),
    last_seen_at timestamptz not null
);

create index device_tokens_household_idx on device_tokens (household_id);
