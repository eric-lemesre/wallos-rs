-- REQ-SYN-006 — idempotence des opérations d'écriture.
-- Un réseau mobile rejoue les requêtes ; sans clé d'idempotence, l'utilisateur découvre des doublons.
-- Une requête d'écriture peut porter une clé (`Idempotency-Key`) ; la première exécution mémorise la
-- réponse **réussie** (2xx) et l'empreinte du corps ; un rejeu à clé+corps identiques renvoie la
-- réponse mémorisée sans nouvel effet de bord ; une clé réutilisée avec un corps différent est un
-- conflit (409). Portée **par utilisateur** (`user_id`) : la clé d'un compte n'interfère jamais avec
-- celle d'un autre (isolation §9). Les seules réponses réussies sont conservées ; sur erreur, la
-- réservation est relâchée pour permettre un nouvel essai.

create table idempotency_keys (
    user_id          uuid        not null references users (id),
    idempotency_key  text        not null,
    request_fingerprint text     not null,
    response_status  smallint    not null,
    response_body    text        not null,
    created_at       timestamptz not null default now(),
    primary key (user_id, idempotency_key)
);
