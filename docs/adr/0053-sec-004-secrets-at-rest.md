# ADR 0053 — Secrets au repos : AES-256-GCM applicatif, clé dérivée d'ENCRYPTION_KEY

- **Statut** : accepté (2026-08-07) — **stratégie de clé validée par Eric** (env var + AES-GCM)
- **Contexte** : REQ-SEC-004 (« Chiffrement au repos des secrets de configuration »), criticality
  **high**, layer `[core, api]`, e2e n-a, oracle design, dépend de NOT-004✓. Dernière exigence du
  backlog. Identifiants SMTP et jetons de messagerie doivent survivre à une fuite de sauvegarde de
  base ; le legacy Wallos stocke tout en clair.

## Décisions

### Dépendances (R6) : `aes-gcm` + `base64`

`aes-gcm` (RustCrypto, MIT/Apache-2.0) : chiffrement **authentifié** AES-256-GCM — toute
altération du texte chiffré est détectée au déchiffrement. `base64` (0.22, MIT/Apache-2.0) pour
l'encodage de stockage. `sha2` était déjà au workspace. Primitives confinées à
`wallos_core::secrets` (layer core exigé par la spec ; fonctions à clé explicite, testables).

### Clé : `ENCRYPTION_KEY` (chaîne libre) → SHA-256 → AES-256

L'opérateur pose une chaîne quelconque (recommandé : 32+ octets aléatoires) ; la clé AES est
`SHA-256(ENCRYPTION_KEY)`. Pas de format imposé, pas de fichier de clé, pas de KMS : adapté à
l'auto-hébergement (choix validé). La rotation de clé n'est pas outillée (supprimer/recréer les
canaux en cas de rotation) ; le préfixe versionné du format laisse la porte ouverte.

### Format stocké : `enc:v1:<base64(nonce)>:<base64(ciphertext+tag)>`

Nonce 96 bits **aléatoire par valeur** (deux secrets identiques ont des chiffrés différents — pas
d'oracle d'égalité en base). Préfixe versionné : détection des valeurs chiffrées, évolution de
schéma possible.

### Champs chiffrés : les secrets seulement

`SECRET_KEYS = [password, bot_token, token, user_key]` — la même liste que la redaction de sortie
(NOT-003/004). Les cibles (URLs, hôte SMTP, chat_id) restent lisibles : diagnostic et affichage de
la carte UI. Chiffrement dans `create_notification_channel` **après** validation ; déchiffrement
dans `channel_from_row` au moment de construire le canal d'envoi (cron, réessai, test de canal).

### Sans clé : refus explicite, jamais de clair silencieux

`ENCRYPTION_KEY` absente ⇒ la création d'un canal **à secrets** est refusée (422 avec message
explicite) ; les canaux sans secret (webhook, Discord) restent créables ; `main` émet un WARN au
démarrage. Un mode « stockage en clair par défaut » aurait vidé l'exigence. Modèle : `CRON_TOKEN`
absent ⇒ endpoint désactivé.

### Lecture : compat clair, échec fermé sur chiffré illisible

Une valeur **non préfixée** est acceptée telle quelle (canaux créés avant SEC-004 — aucune
migration de données ; les recréer les chiffre). Une valeur **chiffrée** sans clé, avec une
mauvaise clé, ou altérée rend le canal inconstructible (`None`) : ignoré par le cron, code
`unreadable-config` au réessai (NOT-007) — un blob chiffré n'est jamais envoyé comme s'il était
le jeton.

### Critère #2 (jamais retourné au client)

Déjà porté par la redaction (`row_to_dto`) ; re-testé sous SEC-004 : ni le clair, ni même la
forme chiffrée ne sortent de l'API — `<redacted>` uniquement.

## Conséquences

- core : module `secrets` (`derive_key`, `encrypt`, `decrypt`, `is_encrypted`, `ENC_PREFIX`) +
  tests (aller-retour, nonce unique, échec fermé, dérivation).
- server : état `EncryptionKey` (Extension, comme `CronToken`), `app_with_db_cron_key` (injection
  de test), chiffrement au create, déchiffrement dans `channel_from_row`, WARN au démarrage.
- Tests d'intégration : secret illisible en base ET utilisable à l'envoi (déchiffrement prouvé
  par le chemin Telegram reçu), redaction totale, 422 sans clé (mais webhook accepté), compat
  clair legacy.
- e2e : `ENCRYPTION_KEY` injectée au serveur de test (playwright).
- Exploitation : documenter `ENCRYPTION_KEY` au déploiement (sa perte rend les canaux à secrets
  irrécupérables — les recréer).
