-- REQ-AUT-002 — sessions serveur : jeton opaque haché (SHA-256, ADR 0018), révocable, expirable.
-- La table ne contient jamais le jeton en clair, seulement son empreinte.

create table sessions (
    token_hash   bytea       primary key,
    user_id      uuid        not null references users (id),
    household_id uuid        not null references households (id),
    created_at   timestamptz not null default now(),
    expires_at   timestamptz not null
);

create index sessions_expires_at_idx on sessions (expires_at);
