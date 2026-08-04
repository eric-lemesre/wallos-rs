# ADR 0028 — REQ-AUT-005 re-cadré en jeton d'API porteur (retrait du volet natif)

- **Statut** : accepté (2026-08-04)
- **Contexte** : OQ-011 (conséquence d'OQ-009 — pas de coquille native pour la parité). REQ-AUT-005
  était `implemented`, bloqué à `verified` par deux critères formulés autour d'une **coquille native**
  absente du périmètre.

## Problème

REQ-AUT-005 (« Jeton d'appareil ») avait deux critères d'acceptation liés au natif :

1. « une authentification **depuis une coquille native** … un jeton propre à l'appareil est émis » ;
2. « stocké côté client **via `PlatformAdapter.secureStore`**, jamais `localStorage` ».

OQ-009 ayant retiré les coquilles natives du périmètre (parité — Wallos n'en a pas), le critère #2
devenait **invérifiable** (aucun `secureStore`), et le #1 était formulé autour d'une modalité inexistante.
Le back-end, lui, est **livré et testé** : endpoint `createDeviceSession`, jetons opaques révocables,
authentification par `Authorization: Bearer` (ADR 0019), avec tests d'intégration (émission → Bearer
fonctionnel, jeton invalide → 401, trio authz, limitation de débit).

## Décision

**Re-cadrer** REQ-AUT-005 en capacité **d'API** (OQ-011, option A) :

- **Critère #1** : une authentification via l'**API** de session d'appareil (`createDeviceSession`)
  émet un **jeton porteur propre à l'appareil**, avec libellé et date de dernière activité.
- **Critère #2** (remplace `secureStore`) : présenté en `Authorization: Bearer`, le jeton authentifie
  **sans cookie** et reste **révocable indépendamment** (REQ-AUT-006).

Le titre devient « Jeton d'appareil (API porteur, révocable) ». La couverture existante satisfait déjà
ces critères ; le scénario e2e `devices.spec.ts` (appairage → jeton émis → listé → révoqué) est
**tagué `@REQ-AUT-005`** en plus de `@REQ-AUT-006`. Promotion `implemented → verified`.

## Conséquences

- Le jeton d'appareil reste utile **hors natif** : clients d'API, scripts, intégrations (cohérent avec
  la promesse d'auto-hébergement). Rien à retirer côté code.
- `PlatformAdapter`/`secureStore`/coquille desktop restent hors périmètre (OQ-009) ; leur nettoyage est
  suivi séparément.
- Si une divergence fonctionnelle réintroduisait un jour une coquille native, le stockage sécurisé
  ferait l'objet d'une **nouvelle** exigence, sans rouvrir celle-ci.
