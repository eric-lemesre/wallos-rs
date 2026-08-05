# ADR 0035 — Récupération incrémentale : curseur `(updated_at, id)`, pagination keyset, flux unifié

- **Statut** : accepté (2026-08-05)
- **Contexte** : REQ-SYN-003 (« récupération incrémentale par curseur »), `oracle: design`, criticality
  high, layer `[api]`, e2e required, dépend de REQ-SYN-001 (horodatage/id) et REQ-SYN-002 (pierres
  tombales).

## Problème

Le client ne doit jamais recharger l'intégralité du jeu de données. Acceptation : (#1) depuis un curseur,
recevoir **créations, modifications et suppressions** postérieures **plus un nouveau curseur** ; (#2) un
delta **dépassant la taille de page** est paginé **de façon stable — aucune entité omise ni dupliquée**.
À trancher : la clé de curseur/pagination, la façon d'unir des sources hétérogènes, et la forme du corps
d'un changement.

## Décision

### Curseur = clé de tri totale `(updated_at, id)`

Un unique [`core::SyncCursor`] `(horodatage, id)` sert **à la fois** de watermark de dernière
synchronisation **et** de position de pagination. La pagination est **keyset** : une page renvoie les
changements dont `(ts, id)` est **strictement supérieur** au curseur (comparaison de tuple Postgres),
triés par `(ts, id)` croissant. L'ordre étant **total** (l'`id` départage les horodatages égaux), deux
pages consécutives ne peuvent ni omettre ni dupliquer une entité (critère #2) — contrairement à un
`OFFSET` qui dérive sous écritures concurrentes. Le curseur est sérialisé en chaîne **opaque**
`<rfc3339 Z>_<uuid>` (suffixe `Z`, jamais `+00:00` qui casserait une query URL).

### Flux unifié en une requête SQL

Les changements proviennent d'une seule requête `UNION ALL` : les **upserts** des quatre entités
possédées (`categories`, `payment_methods`, `payers`, `subscriptions`, clé `updated_at`) et les
**suppressions** (table `tombstones`, clé `deleted_at`), filtrés par foyer (§9), puis
`where (ts, id) > (curseur) order by ts, id limit page+1`. Postgres réalise l'union, le tri global et la
pagination keyset ; la ligne surnuméraire (`+1`) détecte `has_more`. Une entité recréée après suppression
apparaît en `delete` puis `upsert` (ordre des horodatages) — le client applique dans l'ordre.

### Corps d'un changement = ligne persistée (moins `household_id`, argent en chaîne)

Le `payload` d'un `upsert` est la **ligne persistée** sérialisée en JSON (`to_jsonb(row)`), **privée de
`household_id`** (jamais divulgué, §9). Le champ monétaire `subscriptions.amount` est **coercé en chaîne**
(`::text`) : jamais un nombre JSON, qui serait relu en flottant côté client (**R4** — l'argent reste
décimal exact). C'est un choix pragmatique : le corps épouse la **forme de stockage** (noms de colonnes
`cycle_unit`/`category_id`/…), non le DTO d'API. Cela réplique fidèlement l'état sans sérialiseur par
type ; un mapping vers les DTO d'API pourra s'ajouter ultérieurement si un client le requiert. Une
suppression n'a pas de corps (`null`).

### Curseur périmé → resynchronisation complète

`full_resync_required` réutilise `core::requires_full_resync` (ADR 0013) : si le curseur précède la
fenêtre de rétention des pierres tombales, des suppressions ont pu être purgées ; le client repart d'une
**synchronisation complète** (curseur absent = origine) plutôt que d'un delta silencieusement incomplet.
Un curseur absent (première synchro) renvoie toutes les entités vivantes.

## Conséquences

- `core::SyncCursor` (encode/parse/beginning + tests), `storage::sync_changes` (requête unifiée keyset),
  endpoint `getSyncChanges` (`GET /sync/changes?cursor=&limit=`, page bornée 1..=500, défaut 100 ;
  couverture API + authz 100 %).
- Le corps expose la **forme de stockage** : coupler un client à ces noms de colonnes est un compromis
  assumé (relire si un DTO stable de réplication devient nécessaire).
- Débloque REQ-SYN-004 (poussée des modifications locales) et REQ-SYN-005 (résolution de conflit), qui
  s'appuient sur le même curseur/horodatage.
