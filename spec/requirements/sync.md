# Domaine SYN — Synchronisation

> Domaine `oracle: design` intégral : l'application d'origine est une application web sans
> réplication locale. Aucun comportement de référence n'existe, donc **aucune liberté
> d'interprétation n'est laissée à l'agent** : le protocole ci-dessous est prescriptif.

```yaml
---
id: REQ-SYN-001
title: Horodatage de modification et identifiants stables
domain: sync
status: verified
criticality: high
layer: [core, api]
e2e: n-a
oracle: design
rationale: >
  Préalable à toute réplication : sans identifiant stable généré côté client et sans horodatage
  fiable, la résolution de conflit est impossible.
acceptance:
  - given: une entité créée hors ligne
    when: elle est poussée vers le serveur
    then: elle conserve l'identifiant UUID généré par le client
  - given: toute entité répliquée
    when: elle est modifiée
    then: son horodatage de modification est fourni par le serveur, jamais par l'horloge du client
depends_on: []
---
```

```yaml
---
id: REQ-SYN-002
title: Pierres tombales
domain: sync
status: verified
criticality: high
layer: [core, api]
e2e: required
oracle: design
rationale: >
  Sans trace de suppression, un appareil hors ligne réintroduit les entités qu'il croit vivantes.
acceptance:
  - given: une entité supprimée
    when: un appareil se synchronise
    then: il reçoit la pierre tombale et applique la suppression localement
  - given: une pierre tombale plus ancienne que la période de rétention
    when: la purge s'exécute
    then: elle est supprimée, et un appareil absent au-delà de cette période est contraint à une resynchronisation complète
depends_on: [REQ-SYN-001]
---
```

```yaml
---
id: REQ-SYN-003
title: Récupération incrémentale par curseur
domain: sync
status: verified
criticality: high
layer: [api]
e2e: required
oracle: design
rationale: >
  Le client ne doit jamais recharger l'intégralité du jeu de données.
acceptance:
  - given: un curseur de dernière synchronisation
    when: le client demande le delta
    then: il reçoit les créations, modifications et suppressions postérieures, ainsi qu'un nouveau curseur
  - given: un delta dépassant la taille de page
    when: il est récupéré
    then: la pagination est stable et aucune entité n'est ni omise ni dupliquée
depends_on: [REQ-SYN-001, REQ-SYN-002]
---
```

```yaml
---
id: REQ-SYN-004
title: Poussée des modifications locales
domain: sync
status: verified
criticality: high
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Contrepartie de la récupération ; doit tolérer les reprises après coupure réseau.
acceptance:
  - given: un lot de modifications locales
    when: il est poussé puis rejoué à l'identique après une coupure
    then: l'état final est identique à un envoi unique
  - given: un envoi partiellement rejeté
    when: la réponse est reçue
    then: elle identifie précisément les entités en échec, et les autres sont bien appliquées
depends_on: [REQ-SYN-001, REQ-SYN-006]
---
```

```yaml
---
id: REQ-SYN-005
title: Résolution de conflit
domain: sync
status: verified
criticality: high
layer: [core, api]
e2e: required
oracle: design
rationale: >
  Règle prescriptive : dernière écriture gagnante au niveau de l'enregistrement, arbitrée par
  l'horodatage serveur. Aucune fusion champ à champ, aucun CRDT — la complexité serait
  disproportionnée pour un usage mono-utilisateur multi-appareils.
acceptance:
  - given: deux modifications concurrentes de la même entité
    when: elles sont synchronisées
    then: la version portant l'horodatage serveur le plus récent l'emporte intégralement
  - given: une modification perdue par arbitrage
    when: la résolution s'applique
    then: elle est conservée en journal et l'utilisateur peut la consulter
  - given: une suppression concurrente d'une modification
    when: elles sont arbitrées
    then: la suppression l'emporte
depends_on: [REQ-SYN-001, REQ-SYN-002]
---
```

```yaml
---
id: REQ-SYN-006
title: Idempotence des opérations d'écriture
domain: sync
status: verified
criticality: high
layer: [api]
e2e: required
oracle: design
rationale: >
  Un réseau mobile rejoue les requêtes ; sans clé d'idempotence, l'utilisateur découvre des doublons.
acceptance:
  - given: une requête d'écriture portant une clé d'idempotence
    when: elle est rejouée
    then: la réponse est identique et aucun effet de bord supplémentaire n'est produit
  - given: une clé d'idempotence réutilisée avec une charge utile différente
    when: la requête est reçue
    then: elle est rejetée en 409
depends_on: [REQ-SYN-001]
---
```

```yaml
---
id: REQ-SYN-007
title: Fonctionnement hors ligne
domain: sync
status: verified
criticality: high
layer: [ui]
e2e: required
oracle: design
rationale: >
  Justification principale des coquilles natives ; sans mode hors ligne, un simple navigateur suffirait.
acceptance:
  - given: un client sans connectivité
    when: l'utilisateur consulte, crée ou modifie un abonnement
    then: l'opération aboutit localement et l'interface signale l'état non synchronisé
  - given: le retour de la connectivité
    when: la synchronisation se déclenche
    then: elle est automatique et l'utilisateur n'a aucune action à effectuer
depends_on: [REQ-SYN-004, REQ-SYN-005]
---
```

```yaml
---
id: REQ-SYN-008
title: Appairage et synchronisation initiale
domain: sync
status: verified
criticality: medium
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Premier contact d'un appareil avec un compte existant ; cas où les volumes sont les plus élevés.
acceptance:
  - given: un appareil nouvellement appairé
    when: il se synchronise pour la première fois
    then: il récupère l'intégralité des données du compte de façon paginée et reprenable
  - given: une interruption pendant la synchronisation initiale
    when: elle reprend
    then: elle repart du dernier lot appliqué, sans repartir de zéro
depends_on: [REQ-AUT-005, REQ-SYN-003]
---
```
