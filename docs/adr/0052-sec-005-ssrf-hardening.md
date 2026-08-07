# ADR 0052 — SSRF : validation des adresses résolues à la connexion, redirections refusées

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-SEC-005 (« Protection contre la falsification de requête côté serveur »),
  criticality **high**, layer `[api]`, e2e optional, oracle design. Clôt le renvoi documenté depuis
  NOT-005 (ADR 0043) : la garde d'enregistrement `webhook_url_is_safe` acceptait tout nom d'hôte
  non réservé — un nom résolvant vers une adresse interne (ou re-résolvant : *rebinding*)
  contournait la validation.

## Décisions

### Critère #1 : résolveur DNS public-only, au moment de l'appel

`PublicOnlyResolver` (notifier) est branché sur le client HTTP durci commun
(`ClientBuilder::dns_resolver`) : chaque connexion résout via le résolveur système puis **filtre**
les adresses par `ip_is_public` (mêmes plages que la garde d'enregistrement : bouclage, RFC 1918,
link-local — dont métadonnées d'instance —, ULA, CGNAT, etc.). Aucune adresse publique → erreur
typée `NonPublicAddress` (le message ne porte que le nom d'hôte, jamais un jeton). reqwest ne se
connecte qu'aux adresses retournées par le résolveur : il n'y a **pas de fenêtre** entre
validation et connexion (anti-TOCTOU, anti-rebinding).

Défense en profondeur à trois couches : enregistrement (`webhook_url_is_safe`, 422),
résolution (`PublicOnlyResolver`, à chaque appel), redirections (refusées).

### Critère #2 : refuser les redirections plutôt que les re-valider

La politique `redirect::Policy::none()` (posée dès NOT-005) reste : une `3xx` est un échec. « La
validation est appliquée à chaque saut » est satisfait trivialement — zéro saut n'est jamais
suivi. Refuser est plus sûr que suivre-et-valider (pas de chaîne à borner, pas d'état).

### Périmètre

- **Couvert** : tous les canaux HTTP sortants (webhook, Discord, Gotify, Telegram, Pushover) —
  URLs utilisateur comme API fixes passent par le même client.
- **IP littérales** : ne passent pas par le résolveur DNS ; elles sont refusées à
  l'enregistrement, et une IP posée par SQL direct (voie de test) suppose une base déjà
  compromise (position ADR 0046, revue NOT-004 F4). C'est ce qui préserve les récepteurs
  `127.0.0.1` des tests d'intégration.
- **SMTP hors périmètre** : l'exigence vise « les webhooks et la récupération de logos ». Un
  relais SMTP est une infrastructure de l'utilisateur, fréquemment locale/privée en
  auto-hébergement — filtrer son adresse casserait ce cas nominal. Risque d'oracle accepté,
  documenté.
- **Logos** : aucune requête serveur (substitut généré localement, REQ-SUB-015) — rien à durcir.

## Conséquences

- notifier : `PublicOnlyResolver` + `NonPublicAddress`, branchés dans `http_client` ;
  `ip_is_public` sert désormais l'enregistrement ET la connexion.
- Test d'intégration : un canal repointé (SQL, simulant un rebinding) vers `localhost:<port>` —
  nom, pas IP — n'atteint jamais le récepteur local ; l'échec ouvre un suivi de livraison
  (REQ-NOT-007) et sera visible de l'utilisateur.
- Le spec e2e NOT-006 (cible `.invalid`) exerce déjà le chemin d'erreur du résolveur.
