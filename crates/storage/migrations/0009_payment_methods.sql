-- REQ-SUB-011 — moyens de paiement, entité POSSÉDÉE (isolation par foyer, §9/SEC-001). Calque des
-- catégories : chaque moyen de paiement appartient à un foyer ; les repositories filtrent toujours
-- par foyer de l'appelant (`&Actor`). Un abonnement référence au plus un moyen de paiement (optionnel,
-- colonne `payment_method_id` déjà présente sur `subscriptions`).

create table payment_methods (
    id           uuid        primary key,
    household_id uuid        not null references households (id),
    name         text        not null,
    created_at   timestamptz not null default now()
);

create index payment_methods_household_idx on payment_methods (household_id);
