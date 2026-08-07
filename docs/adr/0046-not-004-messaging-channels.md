# ADR 0046 — Messageries tierces : quatre adaptateurs sur l'abstraction de canal, divergences legacy assumées

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-NOT-004 (« Canaux de messagerie tiers »), `oracle: legacy`, criticality medium,
  layer `[api, ui]`, e2e optional. Clôt le cluster des canaux de notification : Telegram, Discord,
  Gotify et Pushover rejoignent le webhook (NOT-005, ADR 0043) et l'e-mail (NOT-003, ADR 0044) sur
  l'abstraction `Channel` unique.

## Problème

Le legacy Wallos offre sept messageries (Telegram, Discord, Gotify, Pushover, Ntfy, PushPlus,
Mattermost, ServerChan), chacune avec sa table SQL, ses endpoints save/test et son bloc d'envoi
dupliqué dans le cron. Quatre décisions : (1) quel **périmètre** de canaux ? (2) quel **contenu** de
message ? (3) comment traiter les **URLs utilisateur** (SSRF) et les **secrets** ? (4) quelles
**divergences** vis-à-vis du legacy ?

## Décision

### Périmètre : les quatre canaux nommés par l'exigence

REQ-NOT-004 nomme Telegram, Discord, Gotify et Pushover — exactement ceux implémentés. Ntfy,
PushPlus, Mattermost et ServerChan (présents dans le legacy récent) sont **hors périmètre** de
l'exigence ; les ajouter suivrait mécaniquement le même patron (une variante d'enum + un validateur).

### Quatre variantes d'enum, un message commun

Chaque messagerie est une **variante de `Channel`** (dispatch fermé, ADR 0043) qui ne diffère que
par son transport (critère #1 « même trait d'envoi ») :

| Canal | Cible | Format (oracle legacy) |
|---|---|---|
| Telegram | `POST {api}/bot{token}/sendMessage` (API fixe) | JSON `{chat_id, text}` |
| Discord | `POST {url}` (webhook utilisateur) | JSON `{content, username?, avatar_url?}` |
| Gotify | `POST {url}/message` (serveur utilisateur) | JSON `{message, priority: 5}` |
| Pushover | `POST {api}/1/messages.json` (API fixe) | formulaire `token`/`user`/`message` |

Le contenu est **identique sur tous les canaux** : `message_text(language, notification)` réutilise
les gabarits localisés de l'e-mail (`email_content`, corps seul — pas de sujet en messagerie).
Langue du compte via `owner_contacts()`, repli anglais (REQ-I18N-004). Le legacy composait un
message par canal avec le nom du payeur ; subtrack unifie (parité de substance : nom, échéance,
jours restants, fin d'essai distincte).

Tous les envois passent par le **client HTTP durci commun** : délai 10 s, redirections **refusées**
(même politique anti-SSRF que le webhook — une `3xx` est un échec).

### URLs utilisateur : même garde SSRF ; API fixes : pas de garde

- **Discord** et **Gotify** reçoivent une URL fournie par l'utilisateur → `webhook_url_is_safe` à
  l'enregistrement (422 sinon), comme le webhook générique et comme le legacy (`ssrf_helper`).
- **Telegram** et **Pushover** ciblent une API publique **fixe** codée en dur — aucune URL
  utilisateur, donc rien à valider. La base d'API n'est **pas** exposée à l'enregistrement (la
  validation jette toute clé inconnue) ; le champ `api_base` n'est lisible par le cron que s'il a
  été posé par SQL direct — voie de test uniquement, inaccessible via l'API.

### Secrets redactés partout

`row_to_dto` redacte `bot_token` (Telegram), `token` (Gotify, Pushover) et `user_key` (Pushover)
en plus du `password` SMTP — aucun secret ne ressort de l'API. Les structs porteuses ont un `Debug`
manuel redacté (pattern `EmailConfig`), donc jamais de secret dans les journaux.

### Divergences legacy assumées

1. **`ignore_ssl` (Gotify) non repris** : accepter des certificats invalides ouvre un MITM sur un
   canal transportant les données d'abonnement ; contraire au durcissement du cluster (revue
   sécurité NOT-005). Un serveur Gotify auto-hébergé doit présenter un certificat valide.
2. **Jeton Gotify dans l'en-tête `X-Gotify-Key`**, pas en query string (`?token=` chez legacy) :
   même API supportée côté Gotify, mais le secret ne fuite plus dans les journaux d'accès.
3. **Schéma de config à plat normalisé** (`bot_token`/`chat_id`, `url`/`username`/`avatar_url`,
   `url`/`token`, `user_key`/`token`) sur le CRUD générique de canal — pas de compat ascendante
   avec les endpoints PHP par-canal du legacy (déjà divergents depuis NOT-005).
4. **Pas d'endpoint « test »** : le legacy a `test*notifications.php` par canal ; non requis par
   l'exigence, différé (rejoindrait REQ-NOT-007 diagnostic).

### Surface API inchangée

Aucun endpoint ajouté : le CRUD générique (NOT-005) accepte quatre `kind` supplémentaires. Les
tests d'autorisation existants couvrent donc l'exigence (mêmes `operation_id`).

## Conséquences

- `wallos-notifier` : variantes `Telegram`, `Discord`, `Gotify`, `Pushover` ; `message_text` (pur) ;
  `http_client`/`ensure_success` partagés (le webhook les réutilise).
- `wallos-server` : quatre validateurs de config (SSRF pour Discord/Gotify) ; `channel_from_row`
  étendu (langue du contact, repli `en`) ; redaction élargie.
- UI : `NotificationChannelsCard` gagne quatre types avec leurs champs (jetons en `type="password"`).
- Tests : notifier (redaction Debug, message localisé, étiquettes) ; intégration (CRUD + redaction,
  422 par canal, envoi bout-en-bout des quatre formats via récepteur local, canal désactivé silencieux) ;
  vitest (un ajout par type, cible affichée).
- Un canal désactivé n'émet **aucune** requête (critère #2) : garanti structurellement par le filtre
  SQL `enabled = true` du cron (inchangé), prouvé par test dédié.
