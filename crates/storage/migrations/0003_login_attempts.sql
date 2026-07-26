-- REQ-AUT-008 — limitation du taux de tentatives d'authentification.
-- Chaque tentative échouée est journalisée avec l'e-mail visé et l'IP source (nullable : inconnue
-- lorsqu'aucun ConnectInfo n'est disponible). Les compteurs par compte et par IP sont dérivés en
-- comptant les lignes dans la fenêtre glissante. `created_at` est fourni par l'appelant (instant
-- injecté, cohérent avec le principe de reproductibilité — cf. sessions).

create table login_attempts (
    id         bigserial   primary key,
    email      text        not null,
    ip         text,
    created_at timestamptz not null
);

create index login_attempts_email_idx on login_attempts (email, created_at);
create index login_attempts_ip_idx on login_attempts (ip, created_at);
