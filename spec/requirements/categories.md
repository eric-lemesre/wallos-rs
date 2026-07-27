# Domaine CAT — Catégories

```yaml
---
id: REQ-CAT-001
title: Gestion des catégories
domain: categories
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Dimension d'analyse principale des statistiques.
acceptance:
  - given: un compte utilisateur
    when: il crée, renomme ou supprime une catégorie
    then: l'opération n'affecte que ses propres catégories
  - given: une catégorie créée
    when: elle est listée
    then: elle est disponible immédiatement dans le formulaire d'abonnement
depends_on: [REQ-SEC-001]
---
```

```yaml
---
id: REQ-CAT-002
title: Catégories par défaut à la création du compte
domain: categories
status: draft
criticality: low
layer: [core, api]
e2e: optional
oracle: legacy
rationale: >
  Évite un premier écran vide ; la liste doit être reprise de l'application d'origine et traduite.
acceptance:
  - given: un compte nouvellement créé
    when: l'utilisateur ouvre le formulaire d'abonnement
    then: un jeu de catégories par défaut est présent, dans la langue du compte
depends_on: [REQ-CAT-001, REQ-I18N-001]
---
```

```yaml
---
id: REQ-CAT-003
title: Suppression d'une catégorie référencée
domain: categories
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Cas limite classique où un agent invente une règle plausible mais divergente.
acceptance:
  - given: une catégorie référencée par au moins un abonnement
    when: sa suppression est demandée
    then: le comportement appliqué est celui capturé sur l'application d'origine, gelé en fixture
  - given: le comportement retenu
    when: la suppression aboutit
    then: aucun abonnement ne référence une catégorie inexistante
depends_on: [REQ-CAT-001]
---
```

```yaml
---
id: REQ-CAT-004
title: Unicité du nom de catégorie par compte
domain: categories
status: draft
criticality: low
layer: [core, api]
e2e: optional
oracle: design
rationale: >
  Deux catégories homonymes rendent les statistiques illisibles.
acceptance:
  - given: une catégorie nommée X
    when: une seconde catégorie X est créée sur le même compte
    then: la création est refusée avec une erreur de validation
  - given: une catégorie X sur un autre compte
    when: X est créée
    then: la création aboutit
depends_on: [REQ-CAT-001]
---
```

```yaml
---
id: REQ-CAT-005
title: Ordre d'affichage des catégories
domain: categories
status: draft
criticality: low
layer: [api, ui]
e2e: optional
oracle: design
rationale: >
  Un ordre non déterministe produit des tests E2E instables.
acceptance:
  - given: plusieurs catégories
    when: elles sont listées
    then: l'ordre est déterministe et identique sur les trois modalités
depends_on: [REQ-CAT-001]
---
```
