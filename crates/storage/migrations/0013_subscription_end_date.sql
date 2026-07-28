-- REQ-SUB-009 — date de fin (annulation programmée) d'un abonnement. Optionnelle : aucune échéance
-- n'est produite au-delà, et l'abonnement apparaît « terminé » (exclu des agrégats) une fois dépassée.
alter table subscriptions add column end_date date;
