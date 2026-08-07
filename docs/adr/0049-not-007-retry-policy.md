# ADR 0049 — Réessai des livraisons : outbox par (canal, lot), intervalle croissant borné, abandon visible

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-NOT-007 (« Politique de réessai et d'abandon »), `oracle: design`, criticality
  medium, layer `[core, api]`, e2e optional, dépend de NOT-002✓. Un canal tiers momentanément
  indisponible ne doit **ni perdre le rappel ni boucler indéfiniment** ; un abandon doit être
  visible par l'utilisateur, pas seulement journalisé.

## Problème

Avant ce cycle, l'envoi était best-effort : un échec était journalisé et perdu. Quatre décisions :
(1) quel **grain** de suivi ? (2) quelle **politique** d'intervalle et de borne ? (3) comment
éviter le double-réessai multi-instances ? (4) comment rendre l'abandon **visible** ?

## Décision

### Grain : (canal, lot du jour), pattern *outbox*

Table `notification_deliveries` (migration 0025), une ligne par `(channel_id, as_of)` portant la
**charge utile sérialisée** du lot raté — le réessai rejoue exactement ce qui aurait dû partir.
Le suivi est **ouvert avant l'envoi** et refermé (supprimé) au succès :

- au **nominal**, aucune trace ne subsiste (pas de bruit, pas de croissance de table) ;
- sur **échec**, la ligne `pending` reste, avec `attempts`, `next_attempt_at` et un code de
  diagnostic redacté (les mêmes codes que l'envoi de test NOT-006 — jamais l'erreur brute) ;
- sur **crash** entre journalisation d'occurrence et envoi (revue NOT-002 F1/F2), la ligne
  `pending` subsiste aussi : la perte silencieuse est impossible. Résidu assumé : un crash entre
  envoi et fermeture produit un doublon au réessai — préférable à une perte pour un rappel.

### Politique pure dans core

`wallos_core::retry_delay_minutes(attempt)` : 1 h, 4 h, 12 h, 24 h, puis `None` = abandon
(`MAX_DELIVERY_ATTEMPTS = 5`, tentative initiale comprise). Fonction pure testée ; la cadence
effective dépend du déclenchement de l'ordonnanceur externe (ADR 0040) — le délai est un plancher.

### Réessai réclamé, jamais doublé

La phase de réessai (fin de `run_reminders`) **réclame** les livraisons dues par un unique
`UPDATE … WHERE status = 'pending' AND next_attempt_at <= now() RETURNING …` qui incrémente
`attempts` et repousse l'échéance : une instance concurrente ne matche plus la condition — même
garantie par la base que NOT-002 (ADR 0048), aucun verrou applicatif. Un canal **désactivé** est
exclu du claim (aucune requête sortante, REQ-NOT-004) sans avancer son compteur : réactivé, il
reprend. Une config devenue illisible abandonne immédiatement (visible) plutôt que de boucler.

### Abandon visible

`GET /notifications/deliveries` (`listNotificationDeliveries`) liste les livraisons `pending` et
`abandoned` du foyer (§9), jointes au type de canal ; `RunRemindersResponse` gagne `retried` /
`abandoned` (observabilité opérateur). L'UI (`NotificationChannelsCard`) affiche la section
« Envois en difficulté » — critère #2 satisfait dans l'interface, pas seulement dans les journaux.

### Hors périmètre

Pas d'acquittement/purge des abandons (l'utilisateur corrige le canal via l'envoi de test NOT-006
puis supprime/recrée ; une purge planifiée pourra venir avec l'exploitation), pas de reprise des
occurrences **antérieures** à la création du suivi, pas de jitter (une seule instance type).

## Conséquences

- core : `retry_delay_minutes` + `MAX_DELIVERY_ATTEMPTS` (purs, testés).
- storage : `notification_deliveries` (migration 0025) + repository (open / claim / resolve /
  record_retry_failure / list).
- server : cron restructuré en outbox + phase de réessai ; endpoint `listNotificationDeliveries`
  (authz ×3) ; `RunRemindersResponse.retried/abandoned`.
- notifier : `ReminderNotification`/`ReminderItem` deviennent `Deserialize` (rejeu du payload).
- UI : section « Envois en difficulté » (statut localisé, `data-status` pour les tests).
