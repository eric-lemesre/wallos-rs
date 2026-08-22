# ADR 0061 — Service statique de l'interface par la feature `fs` de tower-http

- **Statut** : accepté (2026-08-22)
- **Contexte** : REQ-OPS-003 (service de l'interface web par le serveur) exige qu'un seul
  processus serve l'API et l'interface compilée sur la même origine.

## Problème

Servir des fichiers statiques correctement — types MIME, `ETag`/`Last-Modified`, requêtes de
plage, refus de traversée de chemin — est un problème résolu qu'il serait fautif de réimplémenter
à la main : chaque détail omis est un bug de plus à couvrir à 100 %.

## Décision

Activer la feature **`fs`** de `tower-http` (dépendance déjà arbitrée du workspace, utilisée pour
`cors`/`trace`/`compression-gzip`) et servir l'interface par `ServeDir`, avec un repli maison
minimal : document d'entrée pour les routes internes de l'interface, erreur structurée RFC 9457
pour les routes `/api/` inconnues (`crates/server/src/webui.rs`).

R6 vise les dépendances **nouvelles** ; une feature d'une dépendance existante n'en est pas une,
mais elle introduit des crates transitifs (`mime_guess`, `httpdate`) — d'où cet ADR, qui acte le
choix plutôt que de le laisser passer en silence.

## Conséquences

- `cargo-deny`/`audit` couvrent les nouveaux crates transitifs comme le reste.
- La politique de cache (actifs empreinte immuables, document d'entrée `no-cache`) et la
  distinction API/interface restent du code du projet, testées par `crates/server/tests/webui.rs`.
- Le répertoire servi est désigné par `WEBUI_DIR` (référence de configuration, REQ-OPS-004) ;
  son absence laisse l'API seule servie — aucun couplage du serveur au build Vite.
