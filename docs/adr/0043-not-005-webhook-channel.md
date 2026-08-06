# ADR 0043 — Canaux de notification : abstraction fermée + webhook générique, garde SSRF à l'enregistrement

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-NOT-005 (« Webhook générique »), `oracle: legacy`, criticality medium, layer
  `[api, ui]`, e2e optional. Premier canal du cluster Notifications, choisi comme socle de l'abstraction
  d'envoi (NOT-003 e-mail, NOT-004 messageries s'y grefferont). Dépendance **croisée** avec REQ-SEC-005.

## Problème

Trois questions : (1) quelle **abstraction** de canal pour que « les canaux partagent le même trait
d'envoi et ne diffèrent que par leur adaptateur » (NOT-004) ? (2) où appliquer la protection **SSRF**
qu'exige NOT-005 (« une URL pointant vers une adresse interne ou de bouclage est refusée ») ? (3) comment
traiter le **cycle** de dépendances NOT-005 ↔ SEC-005 ?

## Décision

### Abstraction fermée par `enum` (pas de `dyn`, pas de `async_trait`)

L'ensemble des canaux est **connu** (webhook, e-mail, Telegram, Discord, Gotify, Pushover — parité
Wallos). On modélise donc `Channel` comme un **`enum`** (`wallos_notifier`) dont chaque variante porte son
adaptateur, avec une méthode `async fn send(&self, &ReminderNotification)`. Le dispatch fermé évite la
dépendance `async_trait` (que le `dyn` imposerait) et rend l'exhaustivité vérifiée par le compilateur. La
**charge utile** (`ReminderNotification`) est unique et partagée par tous les canaux (critère NOT-004).

### `reqwest` déjà au workspace

L'envoi HTTP réutilise `reqwest` (déjà justifié pour le client et le serveur, ADR 0002) — **aucune
nouvelle dépendance**. NOT-004 (messageries HTTP) le réutilisera ; seul NOT-003 (SMTP) introduira `lettre`.

### Garde SSRF à l'enregistrement (`webhook_url_is_safe`)

La validation refuse, **au moment de créer le canal** : schéma non `http(s)`, hôte de **bouclage**,
**privé** (RFC 1918 / ULA `fc00::/7`), **link-local** (`169.254/16`, `fe80::/10`, dont les métadonnées
d'instance `169.254.169.254`), **non spécifié**, **CGNAT** `100.64/10`, IPv4 mappée en IPv6 pointant vers
une plage interne, et le nom `localhost`. Pur, testé, sans accès horloge/réseau. C'est le **critère #2**
de NOT-005.

### Stockage générique

Table `notification_channels` (§9) : `kind` + `config` (jsonb) + `enabled`. Le serveur ne persiste que
les clés **connues** du type (webhook = `{ url }`), jamais le corps brut du client. Un canal `enabled =
false` n'émet aucune requête (NOT-004). L'émission est câblée dans le cron (`run_reminders`) : les rappels
**nouvellement** émis d'un foyer sont POSTés à ses canaux actifs, **best-effort** — un échec est
journalisé (`tracing::warn`) sans interrompre les autres canaux ni le cron.

### Cycle NOT-005 ↔ SEC-005 : rompu côté NOT-005

`spec` déclare NOT-005 `depends_on` SEC-005 **et** SEC-005 `depends_on` NOT-005 (cycle, comme
NOT-002↔SUB-014, CUR-006↔I18N-003). On **rompt** le cycle en vérifiant NOT-005 avec sa garde **à
l'enregistrement** (suffisante pour son critère #2). SEC-005 (criticality high, `oracle: design`)
**durcira** ensuite la protection sur le **chemin d'appel** : résolution DNS du nom d'hôte vers son IP,
et re-validation **à chaque saut de redirection** — ce que la garde syntaxique à l'enregistrement ne
couvre pas (un nom DNS résolvant vers une IP privée, ou une redirection vers une adresse interne, restent
possibles jusqu'à SEC-005). Limite **documentée**, assumée pour ce vertical.

## Conséquences

- `wallos_notifier` réécrit : `Channel`/`Webhook`/`ReminderNotification`/`webhook_url_is_safe` (+ `reqwest`).
- `notification_channels` (migration 0023, repo storage), 3 opérations CRUD (`listNotificationChannels`,
  `createNotificationChannel`, `deleteNotificationChannel`) avec trio d'autorisation §9.
- Charge utile webhook documentée dans l'OpenAPI (`WebhookReminderPayload`/`WebhookReminderItem`), critère #1.
- UI : `NotificationChannelsCard` (ajout/liste/suppression, signalement du refus SSRF).
- Tests : notifier (SSRF exhaustif + charge utile), intégration (`notification_channels.rs` : CRUD, refus
  SSRF, canal désactivé muet, **envoi bout-en-bout** vers un récepteur local, trio authz ×3).
- **Dette explicite** : SEC-005 (DNS + redirections) reste à faire ; jusque-là la protection est
  syntaxique à l'enregistrement seulement.
