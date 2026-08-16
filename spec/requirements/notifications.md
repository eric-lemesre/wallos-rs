# Domaine NOT — Notifications

> L'ordonnancement vit **côté serveur** (H6). Les clients n'émettent jamais de rappel d'échéance :
> une application desktop fermée ne notifie pas, et deux appareils allumés notifieraient deux fois.

```yaml
---
id: REQ-NOT-001
title: Rappel avant échéance
domain: notifications
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Raison d'être fonctionnelle du produit : ne pas subir un prélèvement oublié.
acceptance:
  - given: un délai de rappel configuré à N jours
    when: une échéance entre dans cette fenêtre
    then: une notification est émise sur les canaux activés
  - given: plusieurs abonnements échéant le même jour
    when: les rappels sont émis
    then: le regroupement appliqué est celui capturé sur l'application d'origine
depends_on: [REQ-SUB-012]
---
```

```yaml
---
id: REQ-NOT-002
title: Idempotence de l'ordonnanceur
domain: notifications
status: verified
criticality: high
layer: [core, api]
e2e: required
oracle: design
rationale: >
  Le piège classique d'un ordonnanceur généré par IA : un redémarrage rejoue la fenêtre et
  l'utilisateur reçoit dix fois le même rappel. C'est le défaut qui fait désinstaller l'application.
acceptance:
  - given: un rappel déjà émis pour une occurrence donnée
    when: l'ordonnanceur s'exécute à nouveau, y compris après redémarrage
    then: aucun second envoi n'a lieu
  - given: plusieurs instances du serveur
    when: elles exécutent l'ordonnanceur simultanément
    then: chaque occurrence donne lieu à exactement un envoi
  - given: une échéance déjà passée lors du premier démarrage
    when: l'ordonnanceur s'exécute
    then: aucun rappel rétroactif n'est émis
depends_on: [REQ-NOT-001, REQ-SUB-014]
---
```

```yaml
---
id: REQ-NOT-003
title: Canal e-mail
domain: notifications
status: verified
criticality: high
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Canal de repli universel, seul disponible sans configuration tierce.
acceptance:
  - given: une configuration SMTP valide
    when: un rappel est émis
    then: le message est envoyé, dans la langue du compte, avec le détail des abonnements concernés
  - given: une configuration SMTP invalide
    when: un rappel est émis
    then: l'échec est journalisé sans exposer les identifiants et n'interrompt pas les autres canaux
depends_on: [REQ-NOT-001, REQ-I18N-001]
---
```

```yaml
---
id: REQ-NOT-004
title: Canaux de messagerie tiers
domain: notifications
status: verified
criticality: medium
layer: [api, ui]
e2e: optional
oracle: legacy
rationale: >
  Reprend les canaux offerts par l'application d'origine, sur une abstraction unique.
acceptance:
  - given: les canaux Telegram, Discord, Gotify et Pushover
    when: ils sont implémentés
    then: ils partagent le même trait d'envoi et ne diffèrent que par leur adaptateur
  - given: un canal désactivé
    when: un rappel est émis
    then: aucune requête sortante ne le concerne
depends_on: [REQ-NOT-001]
---
```

```yaml
---
id: REQ-NOT-005
title: Webhook générique
domain: notifications
status: verified
criticality: medium
layer: [api, ui]
e2e: optional
oracle: legacy
rationale: >
  Point d'extension permettant à l'utilisateur de brancher ses propres automatisations.
acceptance:
  - given: une URL de webhook configurée
    when: un rappel est émis
    then: une requête POST est envoyée avec une charge utile JSON documentée dans OpenAPI
  - given: une URL pointant vers une adresse interne ou de bouclage
    when: elle est enregistrée
    then: elle est refusée, pour prévenir la falsification de requête côté serveur
depends_on: [REQ-NOT-001, REQ-SEC-005]
---
```

```yaml
---
id: REQ-NOT-006
title: Test d'un canal de notification
domain: notifications
status: verified
criticality: medium
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Sans bouton de test, l'utilisateur ne découvre une configuration fautive qu'en manquant un rappel.
acceptance:
  - given: un canal configuré
    when: l'utilisateur déclenche un envoi de test
    then: le résultat est affiché avec un diagnostic exploitable en cas d'échec
depends_on: [REQ-NOT-003, REQ-NOT-004]
---
```

```yaml
---
id: REQ-NOT-007
title: Politique de réessai et d'abandon
domain: notifications
status: verified
criticality: medium
layer: [core, api]
e2e: optional
oracle: design
rationale: >
  Un canal tiers momentanément indisponible ne doit ni perdre le rappel ni boucler indéfiniment.
acceptance:
  - given: un envoi en échec temporaire
    when: la politique s'applique
    then: le réessai suit un intervalle croissant et s'arrête après un nombre borné de tentatives
  - given: un abandon définitif
    when: il survient
    then: il est visible par l'utilisateur dans l'interface, pas seulement dans les journaux
depends_on: [REQ-NOT-002]
---
```

```yaml
---
id: REQ-NOT-008
title: Notification native sur desktop et mobile
domain: notifications
status: draft
criticality: low
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Confort d'usage. Le client se contente d'afficher ce que le serveur a décidé d'émettre.
  Rouverte le 2026-08-16 : l'ADR 0045 l'avait déclarée sans objet faute de coquille native, or
  celle-ci est revenue dans le périmètre (OQ-009 réouverte, ADR 0055). Le volet in-app est acquis
  et le reste ; c'est le premier critère qui redevient exigible.
acceptance:
  - given: une coquille native disposant de la permission
    when: le client reçoit un rappel du serveur
    then: une notification système est affichée via l'adaptateur de plateforme
  - given: une permission refusée
    when: un rappel est reçu
    then: l'application reste fonctionnelle et l'information reste consultable dans l'interface
depends_on: [REQ-NOT-001, REQ-CLT-003]
---
```
