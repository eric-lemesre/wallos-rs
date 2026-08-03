# Domaine SEC — Sécurité et isolation

> `REQ-SEC-001` est l'exigence transversale la plus importante du projet. Elle est rattachée
> à **chaque** opération OpenAPI par la porte CI `authz-coverage` (AGENTS.md §9).

```yaml
---
id: REQ-SEC-001
title: Isolation stricte des données entre comptes
domain: security
status: verified
criticality: high
layer: [core, api]
e2e: required
oracle: design
rationale: >
  Risque numéro un d'un back-end généré : une requête sans clause de propriétaire passe tous les
  tests fonctionnels tout en exposant les données de tous les comptes.
acceptance:
  - given: une entité appartenant au compte A
    when: le compte B authentifié y accède par son identifiant
    then: la réponse est 404, jamais 403 ni 200
  - given: toute opération de l'API
    when: elle est appelée sans authentification
    then: la réponse est 401 et aucune donnée n'est retournée
  - given: une méthode de repository
    when: elle est déclarée
    then: elle exige un contexte d'appelant en paramètre, rendant l'omission non compilable
depends_on: [REQ-AUT-002]
---
```

```yaml
---
id: REQ-SEC-002
title: Format d'erreur uniforme sans fuite d'information
domain: security
status: verified
criticality: high
layer: [api]
e2e: optional
oracle: design
rationale: >
  Les messages d'erreur générés spontanément exposent volontiers des détails d'implémentation.
acceptance:
  - given: une erreur quelconque de l'API
    when: elle est retournée
    then: elle suit le format RFC 9457 avec un type stable et documenté dans OpenAPI
  - given: une erreur interne
    when: elle est retournée
    then: elle ne contient ni trace d'exécution, ni requête SQL, ni chemin de fichier
depends_on: []
---
```

```yaml
---
id: REQ-SEC-003
title: Journalisation sans secret
domain: security
status: verified
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Les journaux sont exportés vers des outils tiers ; un jeton qui s'y trouve est un jeton compromis.
acceptance:
  - given: une requête contenant un mot de passe, un jeton ou une clé de canal de notification
    when: elle est journalisée
    then: ces valeurs sont masquées, y compris en niveau de trace détaillé
  - given: la structure de journalisation
    when: elle est mise en place
    then: le masquage est porté par les types eux-mêmes, pas par une liste de champs à maintenir
depends_on: []
---
```

```yaml
---
id: REQ-SEC-004
title: Chiffrement au repos des secrets de configuration
domain: security
status: draft
criticality: high
layer: [core, api]
e2e: n-a
oracle: design
rationale: >
  Identifiants SMTP et jetons de messagerie sont fournis par l'utilisateur et doivent survivre à
  une fuite de sauvegarde de base.
acceptance:
  - given: un secret de canal de notification enregistré
    when: on inspecte la base de données
    then: la valeur n'est pas lisible en clair
  - given: un secret enregistré
    when: il est relu par l'API
    then: il n'est jamais retourné au client, même à son propriétaire
depends_on: [REQ-NOT-004]
---
```

```yaml
---
id: REQ-SEC-005
title: Protection contre la falsification de requête côté serveur
domain: security
status: draft
criticality: high
layer: [api]
e2e: optional
oracle: design
rationale: >
  Les webhooks et la récupération de logos laissent l'utilisateur choisir une URL sortante.
acceptance:
  - given: une URL résolvant vers une adresse privée, de bouclage ou de métadonnées d'instance
    when: elle est enregistrée ou appelée
    then: la requête est refusée, y compris après redirection
  - given: une redirection en chaîne
    when: elle est suivie
    then: la validation est appliquée à chaque saut, pas seulement à l'URL initiale
depends_on: [REQ-NOT-005, REQ-SUB-015]
---
```

```yaml
---
id: REQ-SEC-006
title: En-têtes de sécurité et politique de contenu
domain: security
status: draft
criticality: medium
layer: [api, ui]
e2e: optional
oracle: design
rationale: >
  La modalité web est exposée publiquement ; les coquilles natives ont leur propre politique de capacités.
acceptance:
  - given: une réponse de la modalité web
    when: elle est émise
    then: elle porte une politique de sécurité de contenu sans directive permissive de script en ligne
  - given: la configuration Tauri
    when: elle est établie
    then: seules les capacités effectivement utilisées sont accordées, et chacune est justifiée
depends_on: []
---
```

---

# Domaine I18N — Internationalisation

```yaml
---
id: REQ-I18N-001
title: Choix et persistance de la langue
domain: i18n
status: verified
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  La langue conditionne aussi le contenu des notifications émises par le serveur, elle ne peut
  donc pas être un simple réglage local du navigateur.
acceptance:
  - given: une langue sélectionnée par l'utilisateur
    when: il se connecte depuis une autre modalité
    then: la même langue est appliquée
  - given: une langue non renseignée
    when: l'utilisateur ouvre l'application
    then: la langue du système est utilisée si elle est supportée
depends_on: [REQ-AUT-001]
---
```

```yaml
---
id: REQ-I18N-002
title: Absence de chaîne littérale dans le code
domain: i18n
status: verified
criticality: medium
layer: [ui]
e2e: n-a
oracle: design
rationale: >
  Un agent produit spontanément des libellés en dur ; sans porte automatique, la traduction
  se dégrade à chaque itération.
acceptance:
  - given: le code de frontend/ui
    when: il est analysé
    then: aucune chaîne destinée à l'affichage n'y figure hors des fichiers de traduction
  - given: une clé de traduction absente du catalogue de référence
    when: elle est utilisée
    then: la construction échoue
depends_on: [REQ-I18N-001]
---
```

```yaml
---
id: REQ-I18N-003
title: Formats de date et de nombre localisés
domain: i18n
status: draft
criticality: medium
layer: [ui]
e2e: required
oracle: design
rationale: >
  Une date au format américain sur une interface française est perçue comme un défaut, et peut
  induire une erreur d'interprétation d'échéance.
acceptance:
  - given: une date d'échéance
    when: elle est affichée
    then: le format suit la locale active, jamais un format codé en dur
  - given: un fuseau horaire différent de celui du serveur
    when: une date est affichée
    then: elle correspond au jour attendu par l'utilisateur, sans décalage d'un jour
depends_on: [REQ-I18N-001, REQ-CUR-006]
---
```

```yaml
---
id: REQ-I18N-004
title: Repli sur la langue de référence
domain: i18n
status: draft
criticality: low
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Une traduction incomplète ne doit pas produire d'interface trouée.
acceptance:
  - given: une clé absente du catalogue de la langue active
    when: elle est résolue
    then: la valeur de la langue de référence est affichée, et l'absence est signalée en construction
depends_on: [REQ-I18N-002]
---
```
