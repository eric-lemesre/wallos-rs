# Domaine STA — Statistiques et agrégats

> Toutes les exigences de ce domaine sont `oracle: legacy` : les valeurs affichées par
> l'application d'origine sur un jeu de données identique constituent la référence.
> Les totaux capturés sont regelés en fixtures et réinjectés dans les tests unitaires de `core`.

```yaml
---
id: REQ-STA-001
title: Normalisation du coût mensuel
domain: statistics
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Indicateur principal du produit. La méthode de normalisation d'un cycle annuel ou
  hebdomadaire vers un mois est une convention, pas une évidence mathématique.
acceptance:
  - given: un abonnement annuel
    when: son coût mensuel normalisé est calculé
    then: la valeur correspond exactement à celle affichée par l'application d'origine sur le même jeu de données
  - given: un abonnement hebdomadaire
    when: son coût mensuel normalisé est calculé
    then: le facteur appliqué est celui capturé sur l'application d'origine, gelé en fixture
depends_on: [REQ-SUB-003, REQ-CUR-005]
---
```

```yaml
---
id: REQ-STA-002
title: Coût annuel normalisé
domain: statistics
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Doit rester cohérent avec le coût mensuel : les deux indicateurs ne peuvent pas diverger.
acceptance:
  - given: un ensemble d'abonnements
    when: coût mensuel et coût annuel sont calculés
    then: la relation entre les deux est celle de l'application d'origine et reste stable
depends_on: [REQ-STA-001]
---
```

```yaml
---
id: REQ-STA-003
title: Exclusion des abonnements non actifs des agrégats
domain: statistics
status: draft
criticality: high
layer: [core, api]
e2e: required
oracle: legacy
rationale: >
  Règle transverse qui doit s'appliquer identiquement à tous les agrégats, sans exception.
acceptance:
  - given: des abonnements désactivés, terminés ou en période d'essai
    when: un agrégat quelconque est calculé
    then: ils en sont exclus selon la règle propre à chaque état
  - given: un abonnement réactivé
    when: les agrégats sont recalculés
    then: il y est immédiatement réintégré
depends_on: [REQ-SUB-008, REQ-SUB-009, REQ-SUB-010]
---
```

```yaml
---
id: REQ-STA-004
title: Répartition par catégorie et par payeur
domain: statistics
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Deux axes d'analyse partageant la même mécanique d'agrégation.
acceptance:
  - given: des abonnements répartis sur plusieurs catégories
    when: la répartition est calculée
    then: la somme des parts est égale au total général, sans écart d'arrondi
  - given: des abonnements sans catégorie ou sans payeur
    when: la répartition est calculée
    then: ils sont regroupés dans une entrée explicite, jamais omis
depends_on: [REQ-STA-001, REQ-CAT-001]
---
```

```yaml
---
id: REQ-STA-005
title: Échéancier des prochains paiements
domain: statistics
status: verified
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Vue de trésorerie à court terme, motif d'usage quotidien.
acceptance:
  - given: une fenêtre de N jours
    when: l'échéancier est demandé
    then: il liste chaque occurrence de paiement attendue dans la fenêtre, y compris plusieurs occurrences d'un même abonnement
  - given: un abonnement se terminant dans la fenêtre
    when: l'échéancier est calculé
    then: aucune occurrence postérieure à sa date de fin n'apparaît
depends_on: [REQ-SUB-012, REQ-SUB-013, REQ-SUB-009]
---
```

```yaml
---
id: REQ-STA-006
title: Évolution du coût sur douze mois
domain: statistics
status: verified
criticality: low
layer: [core, api, ui]
e2e: optional
oracle: design
rationale: >
  Met en évidence les hausses progressives, principal intérêt d'un suivi dans la durée.
acceptance:
  - given: un historique d'abonnements
    when: la série mensuelle est calculée
    then: chaque point reflète les abonnements actifs à cette date, pas l'état courant projeté
depends_on: [REQ-STA-001]
---
```

```yaml
---
id: REQ-STA-007
title: Cohérence entre les agrégats et les filtres actifs
domain: statistics
status: verified
criticality: medium
layer: [ui]
e2e: required
oracle: legacy
rationale: >
  Un total qui ignore le filtre appliqué est une erreur d'interprétation fréquente.
acceptance:
  - given: un filtre appliqué à la liste
    when: les totaux sont affichés
    then: ils portent sur l'ensemble filtré et l'interface l'indique explicitement
depends_on: [REQ-SUB-006, REQ-STA-001]
---
```

```yaml
---
id: REQ-STA-008
title: Détermination des agrégats
domain: statistics
status: verified
criticality: high
layer: [core]
e2e: n-a
oracle: design
rationale: >
  Condition de testabilité : un agrégat dépendant de l'horloge système n'est pas reproductible.
acceptance:
  - given: un jeu de données et une date de référence fournie explicitement
    when: les agrégats sont calculés deux fois
    then: les résultats sont identiques au bit près
  - given: le code de core
    when: on l'analyse
    then: aucun appel direct à l'horloge système n'y figure, la date est toujours un paramètre
depends_on: []
---
```
