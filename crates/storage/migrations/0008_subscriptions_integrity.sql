-- REQ-SUB-004/006 — durcissement d'intégrité (suite revue SUB-006). Défense en profondeur : le domaine
-- (`Subscription::new`, `into_core`) valide déjà montant >= 0 et intervalle > 0, mais la base ne le
-- garantissait pas structurellement — une ligne corrompue fausserait silencieusement le total agrégé.
alter table subscriptions
    add constraint subscriptions_amount_non_negative check (amount >= 0),
    add constraint subscriptions_cycle_interval_positive check (cycle_interval > 0);

-- Index de filtre (REQ-SUB-006) : les listes filtrent par catégorie / payeur / état dans le foyer.
create index subscriptions_household_category_idx on subscriptions (household_id, category_id);
create index subscriptions_household_payer_idx on subscriptions (household_id, payer_id);
create index subscriptions_household_active_idx on subscriptions (household_id, active);
