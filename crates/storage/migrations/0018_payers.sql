-- REQ-SUB-017 — payeurs, entité POSSÉDÉE (isolation par foyer, §9/SEC-001). Calque des moyens de
-- paiement. Un payeur est une **étiquette nominative** (nom seul, aucun login) — parité avec la table
-- `household` de Wallos 5.4.2 (OQ-010, oracle REQ-SUB-017-payer.json). Un abonnement référence au plus
-- un payeur (colonne `payer_id` déjà présente sur `subscriptions`). Pas d'unicité de nom (comme Wallos).
-- La suppression d'un payeur référencé est REFUSÉE (comportement capturé sur Wallos, jamais cascade).

create table payers (
    id           uuid        primary key,
    household_id uuid        not null references households (id),
    name         text        not null,
    created_at   timestamptz not null default now(),
    updated_at   timestamptz not null default now()
);

create index payers_household_idx on payers (household_id);
