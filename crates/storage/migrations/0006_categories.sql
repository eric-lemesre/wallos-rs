-- REQ-CAT-001 — catégories d'abonnements, entité POSSÉDÉE (isolation par foyer, §9/SEC-001).
-- Chaque catégorie appartient à un foyer (`household_id`) ; les repositories filtrent toujours par
-- foyer de l'appelant (`&Actor`), rendant impossible l'accès aux catégories d'un autre compte.

create table categories (
    id           uuid        primary key,
    household_id uuid        not null references households (id),
    name         text        not null,
    created_at   timestamptz not null default now()
);

create index categories_household_idx on categories (household_id);
