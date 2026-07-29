-- REQ-SYN-001 — horodatage de modification **fourni par le serveur**, jamais par l'horloge du client.
-- Chaque entité de contenu répliquée (abonnements, catégories, moyens de paiement) porte un
-- `updated_at` posé par la base (`now()`, horloge serveur) à la création et réécrit à chaque
-- modification. Préalable à la résolution de conflit (chaîne SYN) : sans horodatage fiable côté
-- serveur, la réplication est impossible. L'identifiant (`id`, UUID) reste stable et peut être
-- **généré côté client** (offline-first) puis conservé tel quel.

alter table categories
    add column updated_at timestamptz not null default now();

alter table subscriptions
    add column updated_at timestamptz not null default now();

alter table payment_methods
    add column updated_at timestamptz not null default now();
