# ADR 0018 — Jeton de session opaque haché SHA-256 (dépendance `sha2`)

## Contexte

REQ-AUT-002 ouvre une session à l'authentification. AGENTS.md §9 fige : jeton **opaque** en cookie
`HttpOnly`, révocable immédiatement (JWT interdit). Le jeton est un secret de 256 bits tiré d'un CSPRNG
(`rand::rngs::OsRng`), transmis au client dans le cookie. Le serveur doit le retrouver à chaque
requête pour valider la session.

Stocker le jeton **en clair** en base ferait qu'une fuite de la table `sessions` livrerait des
sessions actives détournables. On stocke donc son **empreinte** ; la vérification par requête doit
rester rapide (lookup indexé), ce qui exclut argon2 (lent, réservé aux mots de passe) et appelle un
hachage cryptographique rapide.

## Décision

- Le jeton de session est **haché en SHA-256** avant stockage : la table `sessions` ne contient que
  `token_hash` (`bytea`), jamais le jeton. Le cookie porte le jeton en clair (sur canal `Secure`).
- Introduire la dépendance **`sha2`** (workspace, R6). Utilisée uniquement côté `server`/`storage`
  pour l'empreinte de jeton ; **jamais** pour les mots de passe (argon2, ADR/§9) ni dans `core`.
- Le jeton brut (256 bits) provient de `OsRng` (`rand`, déjà au workspace) ; sa haute entropie rend
  SHA-256 (non salé) approprié — pas de risque de dictionnaire comme pour un mot de passe.

## Conséquences

- Une fuite de `sessions` ne révèle pas de jeton exploitable (préimage SHA-256 infaisable).
- Révocation immédiate : supprimer la ligne invalide la session (contrairement à un JWT).
- `sha2` est une dépendance de sécurité éprouvée, largement auditée ; pas de surface `core`.

## Liens

- AGENTS.md §0 (R6), §9 (jeton opaque, JWT interdit) ; REQ-AUT-002, REQ-AUT-004.

## Statut

accepted
