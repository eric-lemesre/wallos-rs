# Référence de configuration du serveur

> Généré depuis `crates/server/src/config.rs` (REQ-OPS-004) — ne pas éditer à la main.
> La synchronisation est vérifiée par test ; toute variable lue dans le code doit figurer ici.

| Variable | Rôle | Caractère | Défaut | Secret |
|----------|------|-----------|--------|--------|
| `DATABASE_URL` | URL de connexion PostgreSQL (utilisateur, hôte, base) | Obligatoire | — | oui |
| `LISTEN_ADDR` | Adresse et port d'écoute du serveur HTTP (REQ-OPS-002) | Facultative | `127.0.0.1:3000` | non |
| `WEBUI_DIR` | Répertoire de l'interface web compilée servie par le serveur (REQ-OPS-003) | Facultative | — | non |
| `ENCRYPTION_KEY` | Clé de chiffrement au repos des secrets de canaux (REQ-SEC-004) | Facultative | — | oui |
| `CRON_TOKEN` | Secret d'opérateur autorisant le déclenchement du cron de rappels (REQ-NOT-001) | Facultative | — | oui |
| `SESSION_COOKIE_SECURE` | Attribut Secure du cookie de session (REQ-AUT-004) — `false` réservé aux tests locaux en HTTP | Facultative | `true` | non |
| `SESSION_IDLE_TTL_MINUTES` | Durée d'inactivité (minutes) au-delà de laquelle une session est rejetée (REQ-AUT-004) | Facultative | `30` | non |
| `AUTH_RATELIMIT_MAX_ATTEMPTS` | Tentatives échouées, par compte ou par IP, avant limitation de l'authentification (REQ-AUT-008) | Facultative | `5` | non |
| `AUTH_RATELIMIT_WINDOW_SECONDS` | Largeur (secondes) de la fenêtre glissante de comptage des tentatives (REQ-AUT-008) | Facultative | `900` | non |
| `TOMBSTONE_RETENTION_DAYS` | Rétention (jours) des pierres tombales de synchronisation (REQ-SYN-004, ADR 0013) | Facultative | `30` | non |

Une variable marquée **Secret** n'est jamais journalisée ni restituée dans une erreur :
seul son nom est cité. Une variable facultative absente dont l'absence a une conséquence
fonctionnelle est signalée par un avertissement au démarrage.
