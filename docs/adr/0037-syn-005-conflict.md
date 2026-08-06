# ADR 0037 — Résolution de conflit : dernière écriture gagnante + concurrence optimiste + journal

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-SYN-005 (« résolution de conflit »), `oracle: design`, criticality high, layer
  `[core, api]`, e2e required, dépend de REQ-SYN-001 (id/horodatage) et REQ-SYN-002 (pierres tombales).
  Choix d'Eric : **option A — concurrence optimiste + journal**.

## Problème

Règle prescriptive du dépôt : **dernière écriture gagnante au niveau de l'enregistrement**, arbitrée par
l'horodatage serveur (ni fusion champ à champ, ni CRDT). Acceptation : (#1) deux modifications
concurrentes → la plus récente (horodatage serveur) l'emporte **intégralement** ; (#2) une modification
**perdue par arbitrage** est **conservée en journal**, consultable ; (#3) une **suppression** concurrente
l'emporte sur une modification. Le point non trivial : #2 impose de **détecter** qu'une écriture en a
écrasé une autre — sinon toute édition séquentielle serait journalisée à tort.

## Décision

### Concurrence optimiste : `base_version`

Une opération d'`upsert` peut porter `base_version` = l'`updated_at` que le client **croit modifier**. Le
serveur compare à la version courante (`core::arbitrate`, fonction **pure**) :

- **base concordante**, entité neuve, ou base absente → simple application (édition séquentielle, **pas**
  un conflit) ;
- **base ≠ version courante** → **conflit d'écrasement** : la nouvelle écriture l'emporte (dernière
  arrivée → horodatage serveur le plus récent, #1) **et** la version courante écrasée est **journalisée**
  (`overwritten`, #2).

L'horodatage étant assigné à l'arrivée, la dernière écriture reçue gagne toujours ; le journal capture ce
qu'elle a recouvert. Sans `base_version`, aucune détection — c'est le cas de l'appli web online-first
(écritures directes séquentielles) : pas de bruit dans le journal.

### La suppression l'emporte (#3)

Avant tout `upsert`, l'existence d'une **pierre tombale** est vérifiée (REQ-SYN-002) : si l'entité a été
supprimée concurremment, la **suppression l'emporte** — l'écriture entrante est **écartée** (rejetée) et
journalisée (`deleted_remotely`). L'entité n'est jamais ressuscitée par une modification en retard.

### Journal consultable

Table `conflict_journal` (version perdue en JSON, motif, horodatage), `GET /sync/conflicts` (du plus
récent au plus ancien), purgée à la **même rétention** que les pierres tombales (30 j, ADR 0013,
opportuniste). La version perdue épouse la forme de stockage (comme le delta SYN-003).

### Portée : le chemin de poussée

L'arbitrage vit dans `POST /sync/push` (SYN-004), là où surviennent les conflits **multi-appareils**. Le
`PUT`/`DELETE` interactif (appli web online, une seule source) reste en application directe : une édition
interactive n'est pas un conflit.

## Conséquences

- `core::arbitrate` (pur, 3 issues, tests), `storage::conflict_journal` + `tombstones::exists` +
  `sync_changes::current`, migration `0020_conflict_journal`, endpoint `getSyncConflicts` (authz 100 %).
- La détection dépend de la fourniture de `base_version` par le client : sans elle, LWW s'applique
  toujours mais **sans** journal (choix assumé — l'écrasement silencieux reste possible si le client
  n'implémente pas la concurrence optimiste). C'est le compromis de l'option A pour un usage
  mono-utilisateur multi-appareils.
- Clôt le sous-système **Sync** (SYN-001..006 tous verified).
