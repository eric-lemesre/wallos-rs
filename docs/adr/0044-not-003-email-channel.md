# ADR 0044 — Canal e-mail : dépendance `lettre` (SMTP), destinataire = compte, corps localisé serveur

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-NOT-003 (« Canal e-mail »), `oracle: legacy`, criticality high, layer `[api, ui]`,
  e2e required. Deuxième canal du cluster Notifications, greffé sur l'abstraction de NOT-005 (ADR 0043).
  Feu vert d'Eric pour la dépendance SMTP.

## Problème

Quatre décisions : (1) quelle **dépendance SMTP** (R6) ? (2) qui est le **destinataire** et dans quelle
**langue** ? (3) où **composer** le corps localisé (le serveur n'a pas d'i18n) ? (4) comment traiter les
**secrets** (mot de passe SMTP) et l'échec d'un canal ?

## Décision

### Dépendance : `lettre` (R6)

`lettre` 0.11 est la bibliothèque SMTP de référence en Rust (async, mainteneue). Features retenues :
`tokio1`, `tokio1-rustls-tls`, `smtp-transport`, `builder`, `hostname` — **rustls uniquement, pas
d'OpenSSL** (cohérent avec `reqwest` du dépôt). Confinée au crate `wallos-notifier` ; le serveur ne
dépend **pas** de `lettre` (il valide une adresse via `wallos_notifier::is_valid_email_address`).

### Destinataire = compte, langue = compte

REQ-NOT-003 : « le message est envoyé **dans la langue du compte** ». Le canal e-mail ne stocke donc
**que la connexion SMTP** (`host`, `port`, `username`, `password`, `from`, `starttls`) — jamais le
destinataire ni la langue, résolus **au moment de l'envoi** : `owner_contacts()` renvoie, pour chaque
foyer, `(email, langue)` de l'utilisateur le plus ancien (langue par défaut `en`, REQ-I18N-004). Un
foyer multi-utilisateur adresse le titulaire ; la diffusion à tous les membres est différée (non requise).

### Corps localisé côté serveur (petites templates)

Le serveur n'a pas de framework i18n (l'i18n vit côté client). Plutôt que d'en introduire un, la
composition du corps est une **fonction pure** `email_content(language, notification) -> (sujet, corps)`
dans `wallos-notifier`, avec des gabarits **en/fr** (repli anglais). Suffisant pour le contenu factuel
d'un rappel (sujet + liste « nom : échéance le … »), avec une formulation distincte pour une fin d'essai
(REQ-SUB-010). Testable sans SMTP.

### Secrets et résilience

- Le mot de passe SMTP n'est **jamais** renvoyé : le DTO le **redacte** (`<redacted>`), et `EmailConfig`
  a un `Debug` qui masque identifiant et mot de passe (jamais journalisés — critère « échec journalisé
  sans exposer les identifiants »).
- L'envoi est **best-effort** (comme le webhook) : un échec SMTP (config invalide, serveur injoignable)
  est journalisé sans détail brut et **n'interrompt ni les autres canaux ni le cron** (critère #2). Le
  réessai/diagnostic exploitable relève de REQ-NOT-007 (différé).

### Surface API inchangée

NOT-003 **n'ajoute aucun endpoint** : il étend le CRUD générique de canal (NOT-005) pour accepter
`kind = "email"` avec validation de la config SMTP (champs requis, port 1..=65535, `from` analysable).

## Conséquences

- `lettre` ajouté au workspace (rustls) et au crate `notifier`. `Channel::Email`, `EmailConfig` (Debug
  redacté), `Email`, `email_content` (pur), `compose_email`, `is_valid_email_address`.
- `notification_channels` : `owner_contacts()` (storage) ; le cron construit le canal e-mail avec le
  contact du foyer. `create` valide/normalise la config e-mail ; `row_to_dto` redacte le mot de passe.
- UI : `NotificationChannelsCard` gagne un sélecteur de type + les champs SMTP.
- Tests : notifier (localisation en/fr, fin d'essai, adresse invalide, `Debug` redacté), intégration
  (création e-mail + redaction, config invalide 422, **canal e-mail défaillant n'interrompt pas le
  webhook**), UI (ajout e-mail). Oracle NOT-003 gelé.
- **Différé** : diffusion multi-membres, corps HTML/riche, réessai (NOT-007).
