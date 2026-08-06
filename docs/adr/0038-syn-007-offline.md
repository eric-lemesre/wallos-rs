# ADR 0038 — Fonctionnement hors ligne : outbox local + synchronisation automatique (sans natif)

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-SYN-007 (« fonctionnement hors ligne »), `oracle: design`, criticality high, layer
  `[ui]`, e2e required, dépend de REQ-SYN-004 (push) et REQ-SYN-005 (conflits). La justification
  historique de l'exigence est « le mode hors ligne justifie les coquilles natives » — mais le natif est
  **hors périmètre** (OQ-009, ADR 0015 : mobile v1 = web responsive).

## Problème

Acceptation : (#1) sans connectivité, consulter / créer / modifier **aboutit localement** et l'interface
signale l'état **non synchronisé** ; (#2) au retour du réseau, la synchronisation est **automatique**,
sans action de l'utilisateur. Sans coquille native, comment offrir un hors-ligne crédible dans la
modalité web ?

## Décision

### Outbox local + poussée automatique (pas de service worker)

Un module `frontend/ui/src/sync/outbox.ts` maintient une **file d'attente durable** (`localStorage`) des
écritures effectuées hors ligne. Le hook `useOfflineSync` observe la connectivité (`navigator.onLine` +
événements `online`/`offline`) et, **au retour du réseau**, pousse automatiquement la file via
`POST /sync/push` (REQ-SYN-004) — aucune action utilisateur (#2). Le composant `SyncStatus` affiche
l'état (`synced` / `offline` / `pending N`). **Aucune dépendance nouvelle, aucun service worker** :
choix délibérément léger, cohérent avec une modalité web responsive.

### Écriture optimiste hors ligne

Une création hors ligne génère un `id` **côté client** (REQ-SYN-001, `crypto.randomUUID()`), l'ajoute
**optimistement** à la vue (l'opération aboutit localement, #1) et l'empile dans l'outbox. Au flush
réussi, un événement (`wallos:synced`) invite les vues à recharger l'état serveur. La création de payeur
(`PayersList`) est le premier chemin d'écriture rendu offline-capable ; le mécanisme est réutilisable par
les autres formulaires.

### Portée et limites assumées

- La **consultation** hors ligne s'appuie sur l'état déjà chargé en mémoire ; un cache persistant complet
  (lecture hors ligne après rechargement à froid) relèverait d'un stockage local des entités — hors de
  cette exigence, à ajouter si un vrai usage natif l'impose.
- Après un flush, les rejets éventuels (conflits) sont **enregistrés côté serveur** (journal REQ-SYN-005)
  et ne bloquent pas la file : une fois le lot accepté (HTTP 2xx), l'outbox est vidée.
- L'appli web reste **online-first** : le chemin hors ligne est une capacité additionnelle, pas le mode
  par défaut.

## Conséquences

- Nouveau module `sync/outbox` + hook `useOfflineSync` + composant `SyncStatus` (monté dans la coquille
  web), clés i18n `sync.*`. `PayersList` : création offline-capable.
- Tests : unitaires outbox (empilement, flush/vidage, échec conservé), composant `SyncStatus` (offline,
  attente, poussée auto au retour), e2e `@design` (création hors ligne via `context.setOffline`, synchro
  auto, persistance après rechargement).
- La détection de conflit (REQ-SYN-005) s'active si le client fournit `base_version` ; l'outbox ne le
  fournit pas encore (créations neuves surtout) — à enrichir pour les modifications concurrentes.
