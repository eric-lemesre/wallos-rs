# ADR 0048 — Idempotence de l'ordonnanceur : unicité en base, pas de verrou, pas de rattrapage

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-NOT-002 (« Idempotence de l'ordonnanceur »), `oracle: design`, criticality
  **high**, layer `[core, api]`, e2e required. Le piège visé : un redémarrage ou un double
  déclenchement qui rejoue la fenêtre et bombarde l'utilisateur du même rappel.

## Problème

Trois garanties exigées : (1) une occurrence déjà émise ne repart jamais, y compris après
redémarrage ; (2) plusieurs instances simultanées ⇒ exactement un envoi par occurrence ;
(3) aucune émission rétroactive au premier démarrage. Faut-il un verrou d'ordonnanceur, un état
en mémoire, une fenêtre de rattrapage ?

## Décision : les trois garanties sont structurelles, aucune pièce nouvelle

### (1) Rejeu : journal persistant, pas d'état en mémoire

L'ordonnanceur (ADR 0040 : externe, `POST /internal/run-reminders`) est **sans état** : la seule
mémoire est la table `reminder_log`, avec la contrainte d'unicité
`(household_id, subscription_id, due_date, kind)` (migration 0022). `record_emitted` fait
`insert … on conflict do nothing` et l'envoi n'a lieu que si l'insertion a **gagné**
(`rows_affected > 0`). Un redémarrage ne change rien : chaque exécution reconstruit tout depuis la
base.

### (2) Multi-instances : la contrainte d'unicité EST le verrou

Deux instances concurrentes tentent le même insert ; PostgreSQL en sérialise un seul, l'autre voit
`rows_affected = 0` et n'envoie pas. **Aucun verrou applicatif** (advisory lock, leader election) :
un verrou protégerait l'exécution, pas l'occurrence — c'est l'occurrence qui doit être unique, et
la contrainte le garantit au niveau exact où la course existe. Test d'intégration : deux
applications sur **deux pools distincts** (revue F6 : la course se joue bien dans PostgreSQL),
`tokio::join!`, somme des émissions = 1, un seul POST reçu.

### Risques résiduels (revue kimi F1/F2, amendement 2026-08-07)

Pris isolément, ce mécanisme garantit **au plus un** envoi par occurrence — « exactement un »
suppose l'absence de crash dans la fenêtre entre `record_emitted` et `Channel::send` : un arrêt
brutal à cet instant laisserait une occurrence journalisée jamais transmise (perte silencieuse).
Cette fenêtre est **fermée par REQ-NOT-007** (ADR 0049, pattern *outbox*) : le suivi de livraison
est ouvert **avant** l'envoi et refermé au succès — après un crash, la ligne `pending` subsiste et
la phase de réessai transmet le lot. Le résidu devient l'inverse : un crash entre l'envoi et la
fermeture du suivi peut produire un **doublon** au réessai — pour un rappel, un doublon rare est
préférable à une perte silencieuse (exactly-once strict exigerait une transactionnalité de bout en
bout que les canaux tiers n'offrent pas).

### (3) Anti-rétroactif : le déclenchement exact vaut garde

`core::due_reminders` (pur) ne retient une occurrence que si `days_until == lead` **exactement**
(oracle Wallos, NOT-001). Une échéance passée — premier démarrage, interruption prolongée — n'est
jamais « rattrapée » : la fenêtre d'hier n'existe plus aujourd'hui. C'est un choix assumé, identique
au legacy (cron quotidien sans rattrapage) : un rappel en retard est un rappel faux ; le tableau de
bord (échéancier, REQ-STA-005) montre l'état courant.

### E2E de contrat

Layer `[core, api]` : le scénario Playwright (`@REQ-NOT-002`) exerce l'API (`runReminders` ×2 avec
le secret d'opérateur injecté par la config e2e via `CRON_TOKEN`) — `emitted ≥ 1` puis `0`. Pour
rester déterministe malgré la base e2e partagée, l'abonnement est ancré sur une **date fictive**
(2033) dont le jour du mois diffère par construction de « demain » réel utilisé par les autres
specs ; une barrière de persistance (reload + poll de la liste) précède le premier run.

## Conséquences

- **Aucun code de production nouveau** : annotations `REQ-NOT-002` posées sur les mécanismes
  porteurs (`record_emitted`, `due_reminders`, `run_reminders`) ; tests d'intégration des trois
  critères (redémarrage simulé par reconstruction d'application, course contrôlée, rétroactif) avec
  preuve par récepteur local (compte d'envois sortants) ; spec e2e ; `CRON_TOKEN` injecté au serveur
  e2e.
- Si un jour plusieurs instances doivent se **répartir** le balayage (perf), ce sera un problème de
  partitionnement, pas d'idempotence — la contrainte d'unicité reste la garantie finale.
- Le réessai d'un envoi **échoué** (le journal a retenu l'occurrence mais le canal était en panne)
  relève de REQ-NOT-007 : il devra réutiliser `reminder_log` comme source (marquer l'échec), pas la
  contourner.
