-- REQ-SUB-010 — Période d'essai gratuit. Date de fin d'essai (nullable) : tant que today < trial_end_date
-- l'abonnement est en essai (gratuit, exclu des agrégats de coût) ; à partir de cette date il compte
-- normalement. Concept ABSENT de Wallos 5.4.2 (ADR 0041) : introduit par conception. Aucune contrainte de
-- position vis-à-vis de first_payment (un essai précède typiquement la facturation, mais le modèle ne
-- l'impose pas).
alter table subscriptions add column trial_end_date date;

-- Le rappel de FIN D'ESSAI (REQ-SUB-010, critère #2) est DISTINCT du rappel d'échéance de paiement
-- (REQ-NOT-001). On distingue les deux dans le journal par un `kind` (`payment` / `trial_ending`), et
-- l'unicité anti-doublon inclut désormais ce type (un même abonnement peut recevoir, le même jour, un
-- rappel de paiement ET un rappel de fin d'essai s'ils coïncident).
alter table reminder_log add column kind text not null default 'payment';
alter table reminder_log drop constraint reminder_log_household_id_subscription_id_due_date_key;
alter table reminder_log add constraint reminder_log_unique
    unique (household_id, subscription_id, due_date, kind);

