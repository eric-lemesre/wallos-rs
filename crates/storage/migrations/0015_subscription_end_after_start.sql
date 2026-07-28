-- REQ-SUB-009 — durcissement (suite revue) : la date de fin ne peut précéder le premier paiement
-- (une période de facturation vide n'a pas de sens). Le domaine valide déjà (into_core), la base le
-- garantit structurellement.
alter table subscriptions
    add constraint subscriptions_end_after_start check (end_date is null or end_date >= first_payment);
