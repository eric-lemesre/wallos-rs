# ADR 0034 — Pierres tombales : enregistrement transactionnel, curseur `since`, purge à borne injectée

- **Statut** : accepté (2026-08-05)
- **Contexte** : REQ-SYN-002 (« pierres tombales »), `oracle: design`, criticality high, layer
  `[core, api]`, e2e required, dépend de REQ-SYN-001 (verified). La rétention (30 j, paramétrable) est
  déjà arbitrée par l'ADR 0013 (OQ-004).

## Problème

Sans trace de suppression, un appareil hors ligne réintroduit les entités qu'il croit vivantes.
Acceptation : (#1) un appareil qui se synchronise **reçoit la pierre tombale** et applique la
suppression ; (#2) une pierre tombale **au-delà de la rétention est purgée**, et un appareil absent
plus longtemps est **contraint à une resynchronisation complète**. À trancher : quelles entités,
comment l'enregistrement reste atomique, comment exposer les suppressions, et comment purger/périmer
sans horloge dans la logique.

## Décision

### Entités et enregistrement atomique

Les entités **possédées supprimables** sont `category`, `payment_method`, `payer` (les abonnements n'ont
pas de suppression dure). À chaque suppression **effective**, une pierre tombale
`(household_id, entity_type, entity_id, deleted_at)` est insérée **dans la même transaction** que la
suppression (`storage::tombstones::record`, générique sur l'exécuteur) : jamais de suppression sans
trace, jamais de trace pour une suppression refusée (référencée → 409) ou inexistante (404). `deleted_at`
est **fourni par le serveur** (`default now()`, REQ-SYN-001). Contrainte d'unicité
`(household_id, entity_type, entity_id)` + upsert : recréer puis resupprimer la même entité **rafraîchit**
la pierre tombale.

### Exposition : `GET /sync/tombstones?since=`

Un endpoint dédié renvoie les suppressions **postérieures** au curseur `since` (exclusif, RFC 3339),
ordonnées par `deleted_at` croissant, avec `full_resync_required`, `retention_days` et `as_of` (instant
serveur, à réutiliser comme prochain `since`). Les horodatages sont sérialisés avec suffixe **`Z`** (et
précision microseconde) — jamais `+00:00`, dont le `+` se décode en espace dans une query URL. Le curseur
**générique multi-entités** (récupération incrémentale complète des créations/modifications) relève de
**REQ-SYN-003** (encore `draft`) ; SYN-002 se limite aux suppressions.

### Purge et péremption : bornes injectées (pas d'horloge)

La fenêtre de rétention est lue côté serveur (`TOMBSTONE_RETENTION_DAYS`, défaut 30 j, ADR 0013), jamais
par le client. La logique est **pure** dans `core::sync` : `retention_cutoff(now, days)` et
`requires_full_resync(since, now, days)` reçoivent `now` en paramètre (testables sans horloge, contrainte
ADR 0013 / REQ-STA-008). Le handler purge **de façon opportuniste** à chaque lecture
(`purge_expired(now − rétention)`) — pas d'ordonnanceur (celui de NOT-001 est différé) : la maintenance
se fait au fil des synchronisations. Un curseur `since` antérieur à la borne (ou **absent** = première
synchronisation) déclenche `full_resync_required = true`.

## Conséquences

- Nouveau module `core::sync` (2 fonctions pures + tests), `storage::tombstones` (record/list_since/
  purge_expired + tests), migration `0019_tombstones`, endpoint `getTombstones` (authz 100 %).
- Les trois suppressions (`categories`/`payment_methods`/`payers`) deviennent transactionnelles et
  écrivent une pierre tombale ; leurs suites de tests existantes restent vertes.
- Purge opportuniste (pas d'ordonnanceur) : suffisante tant que les appareils synchronisent
  régulièrement ; un balayage périodique pourra s'ajouter avec l'ordonnanceur de NOT-001 (réutilisera
  `purge_expired`).
- Débloque REQ-SYN-003 (curseur incrémental générique), qui étendra le mécanisme `since` à toutes les
  entités.
