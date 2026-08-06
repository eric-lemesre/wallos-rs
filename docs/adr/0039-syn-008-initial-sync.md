# ADR 0039 — Appairage et synchronisation initiale : jeton d'appareil + drain reprenable (curseur persisté)

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-SYN-008 (« appairage et synchronisation initiale »), `oracle: design`, criticality
  medium, layer `[api, ui]`, e2e required, dépend de REQ-AUT-005 (jeton d'appareil) et REQ-SYN-003
  (récupération incrémentale par curseur).

## Problème

Acceptation : (#1) un appareil **nouvellement appairé** récupère **l'intégralité** des données du compte,
de façon **paginée et reprenable** ; (#2) une **interruption** pendant la synchronisation initiale
**reprend du dernier lot appliqué**, sans repartir de zéro. Que faut-il ajouter, la récupération
incrémentale (SYN-003) existant déjà ?

## Décision

### Appairage = jeton d'appareil (aucun code serveur nouveau)

L'« appairage » est la création d'un **jeton d'appareil** (`POST /device-sessions`, REQ-AUT-005) : le
serveur émet un jeton Bearer opaque, révocable. L'extracteur `AuthActor` accepte **déjà**
`Authorization: Bearer <token>` (comme le cookie) sur toutes les routes protégées — un appareil appairé
peut donc appeler `GET /sync/changes` immédiatement. **Aucun nouvel endpoint** : la synchronisation
initiale est le **cas d'usage « depuis l'origine »** de SYN-003 (curseur absent → toutes les entités
vivantes), paginé par keyset (stable, ni omission ni duplication).

### Synchronisation initiale reprenable (client)

Un module `frontend/ui/src/sync/initialSync.ts` draine le delta depuis le curseur **persisté** (ou
l'origine) : après chaque page, `applyBatch` est appelé **puis** le curseur est écrit en `localStorage`.
Si le processus est interrompu (fermeture, coupure), un nouvel appel **reprend exactement au dernier
curseur persisté** (#2) — jamais de reprise depuis zéro. L'ordre d'application (appliquer avant de
persister le curseur) garantit qu'au pire un lot est **rejoué une fois** ; les upserts étant idempotents
(REQ-SYN-004), l'état final est identique.

## Traçabilité

- **API** (`[api]`) : versant Bearer vérifié en intégration (`pairing.rs`) — un appareil appairé draine
  l'intégralité (paginée), la reprise depuis un curseur ne recouvre pas les éléments déjà vus, un jeton
  invalide est refusé (401). Aucun code serveur ajouté au-delà de la réutilisation d'AuthActor + SYN-003.
- **UI** (`[ui]`) : module `initialSync` (curseur persisté, reprenable) + tests unitaires (drain complet,
  reprise après interruption sans re-fetch, démarrage à l'origine). e2e `@design` : reprise **à travers un
  rechargement de page** (le curseur `localStorage` survit), couverture complète sans doublon.

## Conséquences

- Clôt le sous-système **Sync** (SYN-001..008 tous verified).
- La synchronisation initiale n'écrit pas d'état local persistant côté web (les lots sont appliqués en
  mémoire) — cohérent avec SYN-007 (ADR 0038) : un cache local durable relèverait d'un usage natif, hors
  périmètre.
