# Domaine SUB — Abonnements

> Domaine central du produit. Presque tout est `oracle: legacy` : le comportement de référence
> doit être **capturé** sur l'application d'origine avant implémentation, en particulier les
> règles de calcul d'échéance, qui sont le principal piège du projet.

```yaml
---
id: REQ-SUB-001
title: Modèle de données d'un abonnement
domain: subscriptions
status: verified
criticality: high
layer: [core, api]
e2e: n-a
oracle: legacy
rationale: >
  Fonde le schéma, l'OpenAPI et le client TypeScript généré ; toute omission se propage partout.
acceptance:
  - given: le schéma de l'application d'origine
    when: on établit le modèle cible
    then: il couvre nom, montant, devise, cycle de facturation, date de premier paiement, catégorie, moyen de paiement, payeur, logo, URL, notes, état actif
  - given: un abonnement persisté
    when: on le relit
    then: aucun champ n'est perdu ni normalisé silencieusement
depends_on: []
---
```

```yaml
---
id: REQ-SUB-002
title: Création d'un abonnement
domain: subscriptions
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Parcours utilisateur principal.
acceptance:
  - given: un formulaire correctement renseigné
    when: l'utilisateur valide
    then: l'abonnement est créé, rattaché à son compte, et la prochaine échéance est calculée immédiatement
  - given: un montant négatif ou une devise inconnue
    when: l'utilisateur valide
    then: la création est refusée avec une erreur de validation par champ
depends_on: [REQ-SUB-001, REQ-SUB-003]
---
```

```yaml
---
id: REQ-SUB-003
title: Modèle de cycle de facturation
domain: subscriptions
status: verified
criticality: high
layer: [core, api]
e2e: n-a
oracle: legacy
rationale: >
  Un cycle est un couple (unité, intervalle). Toute la logique d'échéance en dépend.
acceptance:
  - given: les cycles proposés par l'application d'origine
    when: on modélise le type BillingCycle
    then: il exprime au minimum jour, semaine, mois et année avec un intervalle entier strictement positif
  - given: un intervalle nul ou négatif
    when: il est construit
    then: la construction échoue au niveau du type, pas à l'exécution
depends_on: []
---
```

```yaml
---
id: REQ-SUB-004
title: Modification d'un abonnement
domain: subscriptions
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  La modification d'un cycle ou d'une date de départ doit recalculer les échéances de façon prévisible.
acceptance:
  - given: un abonnement existant dont on change le cycle
    when: la modification est enregistrée
    then: la prochaine échéance est recalculée à partir de la date de premier paiement et du nouveau cycle
  - given: une modification concurrente
    when: elle est soumise avec un horodatage antérieur
    then: le conflit est résolu selon REQ-SYN-005
depends_on: [REQ-SUB-002, REQ-SUB-012]
---
```

```yaml
---
id: REQ-SUB-005
title: Suppression d'un abonnement
domain: subscriptions
status: draft
criticality: high
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  La synchronisation multi-appareils impose une suppression traçable ; une suppression physique
  serait invisible pour les autres appareils.
acceptance:
  - given: un abonnement existant
    when: l'utilisateur le supprime
    then: une pierre tombale horodatée est créée et l'abonnement disparaît de toutes les vues
  - given: un abonnement supprimé
    when: un autre appareil se synchronise
    then: il applique la suppression localement
depends_on: [REQ-SYN-002]
---
```

```yaml
---
id: REQ-SUB-006
title: Liste des abonnements et filtres
domain: subscriptions
status: verified
criticality: high
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Vue par défaut de l'application.
acceptance:
  - given: des abonnements de catégories, payeurs et états différents
    when: l'utilisateur applique un filtre
    then: seuls les abonnements correspondants sont affichés et le total affiché reflète le filtre
  - given: plusieurs filtres combinés
    when: ils sont appliqués
    then: la combinaison est conjonctive
depends_on: [REQ-SUB-001]
---
```

```yaml
---
id: REQ-SUB-007
title: Recherche et tri
domain: subscriptions
status: draft
criticality: medium
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Confort d'usage au-delà d'une dizaine d'abonnements.
acceptance:
  - given: une saisie de recherche
    when: elle est appliquée
    then: la correspondance est insensible à la casse et aux diacritiques sur le nom et les notes
  - given: un critère de tri (nom, montant normalisé, prochaine échéance)
    when: il est sélectionné
    then: l'ordre est stable et le tri par montant s'effectue après normalisation en devise de référence
depends_on: [REQ-SUB-006, REQ-STA-001]
---
```

```yaml
---
id: REQ-SUB-008
title: Abonnement désactivé
domain: subscriptions
status: verified
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Un abonnement suspendu doit être conservé sans polluer les totaux ni déclencher de rappels.
acceptance:
  - given: un abonnement désactivé
    when: les statistiques sont calculées
    then: il est exclu de tous les agrégats
  - given: un abonnement désactivé
    when: l'ordonnanceur de notifications s'exécute
    then: aucun rappel n'est émis pour cet abonnement
depends_on: [REQ-SUB-001]
---
```

```yaml
---
id: REQ-SUB-009
title: Date de fin et annulation programmée
domain: subscriptions
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Cas courant : un abonnement résilié mais actif jusqu'à la fin de la période payée.
acceptance:
  - given: un abonnement doté d'une date de fin future
    when: on calcule les échéances
    then: aucune échéance postérieure à la date de fin n'est produite
  - given: un abonnement dont la date de fin est dépassée
    when: la liste est affichée
    then: il apparaît comme terminé et est exclu des agrégats
depends_on: [REQ-SUB-012]
---
```

```yaml
---
id: REQ-SUB-010
title: Période d'essai gratuit
domain: subscriptions
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  L'oubli de fin d'essai est un des motifs d'usage principaux d'un suivi d'abonnements.
acceptance:
  - given: un abonnement en période d'essai
    when: les statistiques sont calculées
    then: son coût n'est pas compté tant que l'essai n'est pas terminé
  - given: un abonnement en période d'essai
    when: la fin d'essai approche du délai de rappel configuré
    then: une notification distincte du rappel d'échéance est émise
depends_on: [REQ-SUB-001, REQ-NOT-001]
---
```

```yaml
---
id: REQ-SUB-011
title: Moyen de paiement
domain: subscriptions
status: verified
criticality: low
layer: [core, api, ui]
e2e: optional
oracle: legacy
rationale: >
  Sert de dimension d'analyse et de filtre.
acceptance:
  - given: la liste de moyens de paiement de l'application d'origine
    when: on constitue le référentiel
    then: chaque abonnement peut référencer au plus un moyen de paiement, optionnel
depends_on: [REQ-SUB-001]
---
```

```yaml
---
id: REQ-SUB-012
title: Calcul de la prochaine échéance pour un cycle mensuel
domain: subscriptions
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  L'utilisateur doit voir la date exacte du prochain prélèvement pour anticiper sa trésorerie.
  Exigence pilote de la stratégie d'oracle : le comportement des fins de mois doit être capturé,
  jamais déduit.
acceptance:
  - given: un abonnement mensuel démarré le 31 janvier
    when: on calcule la prochaine échéance
    then: la date retournée est le 28 ou 29 février selon l'année
  - given: un abonnement mensuel dont l'échéance a été ramenée au 28 février
    when: on calcule l'échéance suivante
    then: elle revient au 31 mars et ne reste pas ancrée au 28
  - given: un abonnement mensuel dont l'échéance tombe un jour de changement d'heure
    when: on calcule la prochaine échéance
    then: l'heure locale de facturation est préservée
depends_on: [REQ-SUB-003]
---
```

```yaml
---
id: REQ-SUB-013
title: Calcul de la prochaine échéance pour les cycles jour, semaine et année
domain: subscriptions
status: verified
criticality: high
layer: [core, api]
e2e: optional
oracle: legacy
rationale: >
  Complète REQ-SUB-012 sur les cycles non mensuels, dont le cas bissextile.
acceptance:
  - given: un abonnement annuel démarré le 29 février
    when: on calcule la prochaine échéance
    then: la règle appliquée est celle capturée sur l'application d'origine, gelée en fixture
  - given: un abonnement hebdomadaire
    when: on calcule les échéances sur un an
    then: aucune dérive de jour de la semaine n'est observée
depends_on: [REQ-SUB-003, REQ-SUB-012]
---
```

```yaml
---
id: REQ-SUB-014
title: Rattrapage des échéances passées
domain: subscriptions
status: draft
criticality: high
layer: [core, api]
e2e: required
oracle: legacy
rationale: >
  Un client hors ligne plusieurs semaines doit converger sans produire de rappels rétroactifs en rafale.
acceptance:
  - given: un abonnement dont plusieurs échéances sont dépassées
    when: le calcul est relancé
    then: la prochaine échéance retournée est strictement postérieure à la date courante
  - given: le même abonnement
    when: l'ordonnanceur s'exécute
    then: aucune notification n'est émise pour les échéances déjà passées
depends_on: [REQ-SUB-012, REQ-NOT-002]
---
```

```yaml
---
id: REQ-SUB-015
title: Logo d'abonnement
domain: subscriptions
status: verified
criticality: low
layer: [api, ui]
e2e: optional
oracle: design
rationale: >
  Confort visuel ; la récupération automatique depuis un service tiers pose une question de
  confidentialité qui doit rester un choix explicite de l'utilisateur.
acceptance:
  - given: un abonnement sans logo
    when: il est affiché
    then: un substitut déterministe est généré localement, sans appel réseau
  - given: la recherche automatique de logo désactivée
    when: un abonnement est créé
    then: aucune requête n'est émise vers un service tiers
depends_on: [REQ-SUB-001]
---
```

```yaml
---
id: REQ-SUB-016
title: Import et export des données
domain: subscriptions
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  Condition de réversibilité, et seul chemin de migration depuis l'application d'origine.
acceptance:
  - given: un export JSON complet
    when: il est réimporté dans un compte vierge
    then: l'état obtenu est identique à l'original, échéances recalculées comprises
  - given: un fichier d'export issu de l'application d'origine
    when: il est importé
    then: les abonnements, catégories et devises sont créés, et un rapport liste les lignes rejetées
depends_on: [REQ-SUB-001, REQ-CAT-001]
---
```

```yaml
---
id: REQ-SUB-017
title: Rattachement à un payeur
domain: subscriptions
status: draft
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Permet la répartition des dépenses au sein d'un foyer sans créer de comptes distincts.
acceptance:
  - given: plusieurs payeurs déclarés sur le compte
    when: un abonnement leur est rattaché
    then: les statistiques par payeur reflètent le rattachement
  - given: un payeur supprimé alors qu'il est référencé
    when: la suppression est demandée
    then: elle est refusée ou les abonnements sont réaffectés selon le comportement capturé sur l'application d'origine
depends_on: [REQ-SUB-001, REQ-STA-004]
---
```
