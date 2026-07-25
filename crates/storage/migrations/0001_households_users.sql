-- REQ-AUT-001 — socle multi-utilisateur : foyers et comptes.
-- L'unité de propriété/isolation est le foyer (ADR 0012) ; tout utilisateur y est rattaché.

create extension if not exists citext;

create table households (
    id         uuid        primary key,
    created_at timestamptz not null default now()
);

create table users (
    id            uuid        primary key,
    household_id  uuid        not null references households (id),
    email         citext      not null unique check (char_length(email::text) between 3 and 254),
    password_hash text        not null,
    created_at    timestamptz not null default now()
);

create index users_household_id_idx on users (household_id);
