# Domaine CUR — Devises et montants

> Domaine à haut risque : c'est là que la génération par IA produit les erreurs les plus
> silencieuses (flottants, arrondis, taux périmés). Couverture par mutation obligatoire.

```yaml
---
id: REQ-CUR-001
title: Devise de référence du compte
domain: currencies
status: accepted
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Toute agrégation multi-devises s'exprime dans cette devise.
acceptance:
  - given: un compte dont la devise de référence est modifiée
    when: les statistiques sont recalculées
    then: tous les agrégats sont exprimés dans la nouvelle devise, sans altérer les montants saisis
  - given: un abonnement saisi en devise étrangère
    when: il est relu
    then: le montant et la devise d'origine sont conservés à l'identique
depends_on: []
---
```

```yaml
---
id: REQ-CUR-002
title: Représentation décimale des montants
domain: currencies
status: accepted
criticality: high
layer: [core, api]
e2e: n-a
oracle: design
rationale: >
  Règle R4 d'AGENTS.md. Un f64 dans une addition de montants est un défaut, pas un compromis.
acceptance:
  - given: le code de production des crates core, storage, server
    when: on l'analyse
    then: aucun type flottant n'apparaît dans un chemin manipulant un montant
  - given: une somme de montants à deux décimales
    when: elle est calculée
    then: le résultat est exact, sans erreur de représentation
  - given: un montant transitant par l'API
    when: il est sérialisé
    then: il est transmis en chaîne décimale, jamais en nombre JSON
depends_on: []
---
```

```yaml
---
id: REQ-CUR-003
title: Récupération des taux de change
domain: currencies
status: accepted
criticality: high
layer: [api]
e2e: optional
oracle: design
rationale: >
  Nécessaire à l'agrégation multi-devises ; dépend d'un tiers, donc faillible par construction.
acceptance:
  - given: un fournisseur de taux configuré
    when: la mise à jour périodique s'exécute
    then: les taux sont persistés avec leur date de validité et leur source
  - given: aucun fournisseur configuré
    when: un abonnement en devise étrangère est agrégé
    then: l'application reste fonctionnelle et signale l'agrégat comme partiel
depends_on: [REQ-CUR-001]
---
```

```yaml
---
id: REQ-CUR-004
title: Mode dégradé en cas d'échec du fournisseur de taux
domain: currencies
status: accepted
criticality: high
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  Un agent génère spontanément un chemin d'erreur qui renvoie zéro ou panique ; les deux sont
  inacceptables sur un écran de suivi de dépenses.
acceptance:
  - given: un fournisseur de taux indisponible
    when: une agrégation est demandée
    then: le dernier taux connu est utilisé et l'interface indique sa date
  - given: aucun taux connu pour une devise
    when: une agrégation est demandée
    then: le montant concerné est exclu et l'agrégat est explicitement signalé comme incomplet, jamais silencieusement à zéro
depends_on: [REQ-CUR-003]
---
```

```yaml
---
id: REQ-CUR-005
title: Règle d'arrondi
domain: currencies
status: accepted
criticality: high
layer: [core]
e2e: n-a
oracle: legacy
rationale: >
  L'arrondi doit être appliqué une seule fois, à l'affichage, jamais en cours d'agrégation.
acceptance:
  - given: une somme de montants convertis
    when: elle est calculée
    then: la conversion conserve la précision maximale et l'arrondi n'intervient qu'au formatage
  - given: un montant à arrondir
    when: le formatage s'applique
    then: la règle est l'arrondi bancaire, et le nombre de décimales dépend de la devise
depends_on: [REQ-CUR-002]
---
```

```yaml
---
id: REQ-CUR-006
title: Formatage localisé des montants
domain: currencies
status: accepted
criticality: medium
layer: [ui]
e2e: required
oracle: design
rationale: >
  Séparateurs et position du symbole dépendent de la locale, pas de la devise.
acceptance:
  - given: un montant en euros affiché en locale fr-FR puis en-US
    when: il est formaté
    then: les séparateurs et la position du symbole diffèrent, la valeur non
  - given: une devise sans sous-unité
    when: un montant est formaté
    then: aucune décimale n'est affichée
depends_on: [REQ-CUR-005, REQ-I18N-003]
---
```

```yaml
---
id: REQ-CUR-007
title: Référentiel des devises supportées
domain: currencies
status: accepted
criticality: medium
layer: [core, api, ui]
e2e: optional
oracle: legacy
rationale: >
  Un code devise libre ouvre la porte aux données incohérentes.
acceptance:
  - given: un code devise hors référentiel ISO 4217
    when: il est soumis
    then: la validation échoue côté serveur
  - given: le référentiel
    when: il est exposé à l'interface
    then: il inclut le nom localisé, le symbole et le nombre de décimales
depends_on: []
---
```
