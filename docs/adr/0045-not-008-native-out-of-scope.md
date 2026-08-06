# ADR 0045 — Notification native (desktop/mobile) hors périmètre ; le rappel reste consultable dans l'UI web

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-NOT-008 (« Notification native sur desktop et mobile »), `oracle: design`,
  criticality low, layer `[ui]`, e2e optional. Dépend d'une **coquille native** (`PlatformAdapter.
  notifications`).

## Problème

NOT-008 demande qu'une **coquille native** disposant de la permission affiche une notification système
via `PlatformAdapter.notifications` quand le client reçoit un rappel du serveur (critère #1), et que
l'application reste fonctionnelle avec l'information **consultable dans l'interface** si la permission
est refusée (critère #2).

## Décision : appliquer OQ-009 (natif hors périmètre)

Le responsable du dépôt a tranché (OQ-009, 2026-08-04) : la cible est la **parité** avec l'application
d'origine, or **Wallos n'a ni desktop ni mobile natifs**. Il n'y a **pas de coquille native** ; la
modalité mobile est la **PWA responsive** (ADR 0015). C'est la même décision qui a déjà re-cadré
REQ-SEC-006 (ADR 0032, « capacités Tauri hors périmètre ») et REQ-AUT-005 (ADR 0028).

En conséquence, le **critère #1** de NOT-008 (notification système via une coquille native) est **hors
périmètre** — il n'y a pas de `PlatformAdapter` natif à qui déléguer. Le **critère #2** est en revanche
**déjà satisfait** par l'interface web : les rappels dus sont **consultables dans l'application** via la
carte `RemindersCard` (REQ-NOT-001), qui liste les rappels du jour regroupés par compte. L'application
reste pleinement fonctionnelle sans notification système.

NOT-008 passe donc à **`verified`** au titre de la partie **applicable** (consultation in-app), la partie
native étant explicitement descopée — cohérent avec le traitement de SEC-006/AUT-005. Aucun code nouveau :
la surface de consultation existe déjà.

## Conséquences

- REQ-NOT-008 → `verified` (spec + lock), sans implémentation native. Si une divergence fonctionnelle
  future justifie une coquille native (réouverture d'OQ-009), un nouvel ADR rétablira le critère #1.
- Aucune dépendance ni code ajoutés. La consultation in-app des rappels reste portée par `RemindersCard`
  (NOT-001).
- Rappel : le **serveur** décide et émet (NOT-001 + canaux NOT-003/005…) ; le client se contente
  d'afficher — l'absence de notification système ne masque aucune information.
