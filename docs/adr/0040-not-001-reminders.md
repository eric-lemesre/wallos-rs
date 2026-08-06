# ADR 0040 — Rappels avant échéance : cron déclenché par endpoint, déclenchement exact, regroupement par compte

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-NOT-001 (« rappel avant échéance »), `oracle: legacy`, criticality high, layer
  `[core, api, ui]`, e2e required, dépend de REQ-SUB-012. Première exigence du domaine Notifications ;
  elle en débloque toute la famille. Décision d'architecture arbitrée par Eric : **ordonnanceur =
  endpoint déclenché par un cron externe** (option A).

## Problème

Acceptation : (#1) un délai de rappel configuré à N jours → quand une échéance **entre dans la fenêtre**,
une notification est **émise** ; (#2) plusieurs abonnements échéant le même jour → **regroupement**
capturé sur l'application d'origine. Trois choses à trancher : le **déclencheur** (comment les rappels
partent), la **règle exacte** (fenêtre vs jour précis), et le **regroupement**.

## Décision

### Déclencheur : endpoint `POST /internal/run-reminders` + cron externe

Conformément à Wallos (crontab exécutant `sendnotifications.php`), subtrack expose un endpoint
**d'opérateur** que déclenche un **ordonnanceur externe** (cron OS / conteneur), plutôt qu'une tâche de
fond en-process. Runtime simple (pas de boucle longue à arrêter proprement), fidèle au modèle d'origine,
et testable de façon déterministe (`as_of` injectable, aucun accès horloge dans le domaine —
REQ-STA-008). L'endpoint balaie **tous les foyers**, il n'est donc **pas** derrière `AuthActor`
(per-foyer) : il est authentifié par un **secret d'opérateur** `X-Cron-Token` (configuré côté serveur
via `CRON_TOKEN`, injecté en extension de requête). **Aucun secret configuré → endpoint désactivé
(404)** : jamais ouvert par défaut.

### Règle : déclenchement **exact** (oracle legacy)

Comportement capturé sur Wallos 5.4.2 (`sendnotifications.php`) et **gelé** dans
`e2e/fixtures/oracles/REQ-NOT-001-reminders.json` : un abonnement **actif** déclenche un rappel quand le
nombre de jours calendaires jusqu'à sa prochaine échéance est **exactement** égal au délai
(`difference === daysToCompare`), **pas** pour toute la fenêtre `[0, N]`. Un cron quotidien émet ainsi
chaque rappel une fois, le jour J−N. Règle isolée dans le domaine pur `core::due_reminders`. Le délai est
**par compte** (`households.reminder_lead_days`, défaut 1, parité `notification_settings.days`) ;
l'override par-abonnement de Wallos (`notify_days_before`) est **différé** (le cœur le supporte via le
délai porté par chaque candidat).

### Regroupement : un rappel par compte (critère #2)

Wallos construit **un message par utilisateur** listant tous les abonnements dus ce jour-là (indexés par
`payer_user_id`). subtrack regroupe les rappels dus **par foyer** (un compteur `accounts_notified`) et
conserve le rattachement payeur dans chaque entrée. La vue `GET /reminders` en est le pendant lisible.

### Émission et idempotence

Chaque rappel émis est **journalisé** (`reminder_log`, unicité `(foyer, abonnement, échéance)`) : une
ré-exécution du cron le même jour **ne ré-émet pas** (compteur `emitted` = nouveautés seulement). C'est
la **graine** de REQ-NOT-002 (idempotence de l'ordonnanceur, exigence distincte). Le **canal** concret
d'émission (e-mail, messageries, webhook) relève de REQ-NOT-003/004/005 : NOT-001 se limite à la
**détection + journalisation** (émission « enregistrée »), le crate `notifier` restant l'abstraction de
canal à étoffer.

## Conséquences

- `core::reminders` (règle pure + tests), migration `0021` (`reminder_lead_days` + `reminder_log`),
  `storage::reminders`, endpoints `getReminderSetting`/`setReminderSetting` (réglage), `getReminders`
  (vue du jour), `runReminders` (cron). UI `RemindersCard`. Couverture authz 100 % (trio par opération,
  y compris le cron via son secret).
- **Nouveau motif d'auth** : secret d'opérateur pour un endpoint cross-foyer, injecté en extension
  (`CronToken`) — testable sans variable d'environnement (`app_with_db_and_cron`).
- Débloque la famille Notifications (NOT-002..008) et SEC-004/005, SUB-010/014, STA-003.
- **Différé (documenté)** : override de délai par-abonnement, canaux de livraison réels, envoi
  effectif — chacun est une exigence dédiée à venir.
