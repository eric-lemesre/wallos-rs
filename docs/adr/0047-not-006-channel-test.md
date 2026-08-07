# ADR 0047 — Test d'un canal : envoi sur le canal enregistré, diagnostic par code stable

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-NOT-006 (« Test d'un canal de notification »), `oracle: legacy`, criticality
  medium, layer `[api, ui]`, e2e **required**. Dépend de NOT-003✓ et NOT-004✓ — tous les types de
  canaux existent, le test les couvre uniformément.

## Problème

Le legacy Wallos a un endpoint `test*notifications.php` **par canal**, qui teste la configuration
**du formulaire** (avant sauvegarde) et renvoie `{success, message}` avec un message traduit —
parfois l'erreur brute du transport. Trois décisions : (1) tester la config du formulaire ou le
canal **enregistré** ? (2) quel **contenu** de test ? (3) quel **diagnostic** renvoyer sans fuiter
de secret ?

## Décision

### Un seul endpoint, sur le canal enregistré

`POST /notifications/channels/{id}/test` (`testNotificationChannel`) teste un canal **déjà
enregistré** du foyer (404 hors foyer, §9). Divergence assumée vs legacy (test de la config du
formulaire) :

- le flux subtrack est « ajouter → tester » (le CRUD est générique, l'ajout est immédiat) ;
- tester une config ad hoc obligerait à re-valider la garde SSRF dans un second chemin — un
  endpoint qui POST vers une URL arbitraire du corps de requête est exactement la primitive SSRF
  qu'on refuse de créer ;
- un canal **désactivé** reste testable : le test sert à valider une configuration avant de
  l'activer (le filtre `enabled` ne concerne que le cron).

### Contenu : notification factice sur le chemin d'envoi réel

Le test construit une `ReminderNotification` **factice** (abonnement « Test subscription » échéant
dans 5 jours — esprit de la fake subscription du legacy) et l'envoie via `Channel::send`, le même
chemin que le cron : mêmes adaptateurs, mêmes gabarits localisés (langue du compte via
`owner_contact`), même client durci. Tester, c'est exercer exactement ce qui sera émis. Un webhook
reçoit donc la charge JSON documentée (`reminder_count: 1`, id nil).

### Diagnostic : code stable, jamais l'erreur brute

Réponse `200 {ok, code, http_status?}`. Codes : `sent`, `http-status` (+ statut de la cible),
`timeout`, `connection-failed`, `smtp-failed`, `send-failed`. La classification
(`diagnose_send_error`, notifier) downcaste l'erreur (`UnexpectedStatus` typée, `reqwest::Error`,
erreurs `lettre`) ; le **texte brut n'est jamais renvoyé** — il peut contenir l'URL cible (donc le
jeton de bot Telegram, interpolé dans le chemin) ou des détails SMTP. La localisation du message se
fait côté client à partir du code (le serveur n'a pas d'i18n, même choix qu'ADR 0044).

### E2E sans dépendance externe

Le scénario Playwright (`@REQ-NOT-006`) enregistre un webhook vers un domaine du TLD réservé
`.invalid` (RFC 6761 : résolution toujours en échec) et vérifie l'affichage du diagnostic d'échec.
Le cas de succès est couvert en intégration (récepteur local par repoint SQL — la garde SSRF
interdit d'enregistrer une URL de bouclage via l'API).

## Conséquences

- notifier : `UnexpectedStatus(u16)` (erreur typée), `diagnose_send_error` (pur).
- storage : `NotificationChannelRepository::get` (isolation §9) et `owner_contact(household_id)`.
- server : handler + route ; `channel_from_row` passe `pub(crate)` (partagé cron/test).
- proto : `TestNotificationChannelResponse { ok, code, http_status? }`.
- UI : bouton « Tester » par ligne, résultat localisé (`data-ok` pour l'e2e et les tests).
- Le legacy ntfy/pushplus/mattermost/serverchan n'ayant pas de canal subtrack (ADR 0046), leurs
  endpoints de test n'ont pas d'équivalent — cohérent.
