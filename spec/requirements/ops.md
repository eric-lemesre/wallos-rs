# Domaine OPS — Exploitation et déploiement

> L'argument central du produit est l'**auto-hébergement** : les données restent chez l'utilisateur.
> Cette promesse n'est tenue que si un tiers peut **installer**, **configurer**, **surveiller** et
> **restaurer** l'application sans lire le code source. Le domaine OPS couvre ce dernier maillon —
> il ne décrit aucun comportement métier, seulement le contrat entre le logiciel et son exploitant.

```yaml
---
id: REQ-OPS-001
title: Endpoint de santé
domain: ops
status: verified
criticality: low
layer:
- api
e2e: optional
oracle: design
rationale: >
  Fournir un point de contrôle simple permettant de vérifier que le serveur est démarré
  et répond aux requêtes HTTP.
acceptance:
  - given: le serveur est démarré
    when: on requête GET /health
    then: la réponse est 200 avec le corps "ok"
depends_on: []
---
```

```yaml
---
id: REQ-OPS-002
title: Adresse et port d'écoute configurables
domain: ops
status: draft
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Le serveur fixe son écoute en dur sur la boucle locale. Dans un conteneur, un service géré ou
  derrière un reverse-proxy sur une autre interface, il est simplement injoignable : aucun
  déploiement n'est possible sans recompilation.
acceptance:
  - given: aucune variable d'écoute renseignée
    when: le serveur démarre
    then: il écoute sur la boucle locale, port 3000, comme aujourd'hui
  - given: une adresse et un port fournis par l'environnement
    when: le serveur démarre
    then: il écoute sur cette adresse et ce port, et le journal de démarrage indique l'écoute effective
  - given: une valeur d'écoute syntaxiquement invalide
    when: le serveur démarre
    then: il s'arrête immédiatement en nommant la variable fautive, plutôt que de se rabattre
      silencieusement sur une valeur par défaut
depends_on: [REQ-OPS-001]
---
```

```yaml
---
id: REQ-OPS-003
title: Service de l'interface web par le serveur
domain: ops
status: draft
criticality: high
layer: [api, ui]
e2e: optional
oracle: design
rationale: >
  Exiger un second serveur web pour les fichiers de l'interface double la surface d'installation et
  introduit une origine distincte, donc du CORS et des cookies tiers. Un déploiement auto-hébergé
  doit tenir en un seul processus servant l'API et l'interface sur la même origine.
acceptance:
  - given: une interface compilée présente
    when: on requête la racine ou une route interne de l'interface
    then: l'application est servie, le routage côté client se repliant sur le document d'entrée
  - given: une route d'API inexistante
    when: elle est requêtée
    then: la réponse est une erreur applicative structurée, jamais le document de l'interface
  - given: les en-têtes de sécurité en vigueur
    when: des fichiers d'actifs sont servis
    then: ces en-têtes restent appliqués, et les actifs versionnés par empreinte sont mis en cache
      durablement tandis que le document d'entrée ne l'est pas
  - given: aucune interface compilée présente
    when: le serveur démarre
    then: l'API reste pleinement fonctionnelle et l'absence d'interface est signalée au démarrage
depends_on: [REQ-OPS-001, REQ-SEC-006]
---
```

```yaml
---
id: REQ-OPS-004
title: Configuration d'exécution documentée et validée au démarrage
domain: ops
status: draft
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Le serveur lit aujourd'hui des variables décisives pour la sécurité — clé de chiffrement au repos,
  jeton d'ordonnanceur, drapeau de cookie sécurisé, fenêtre de limitation des tentatives — sans
  qu'aucune ne soit décrite hors du code. Une configuration incomplète se manifeste alors tard, par
  un refus opaque en pleine utilisation. L'exploitant doit connaître le contrat avant de démarrer.
acceptance:
  - given: l'ensemble des variables d'environnement lues par le serveur
    when: la référence de configuration est produite
    then: chacune y figure avec son rôle, son caractère obligatoire ou facultatif et sa valeur par
      défaut, et une porte automatique échoue si une variable lue dans le code n'y est pas décrite
  - given: une variable obligatoire absente ou invalide
    when: le serveur démarre
    then: il s'arrête sans servir de requête, en nommant la variable et en énonçant l'attendu, sans
      jamais restituer la valeur reçue
  - given: une configuration incomplète mais tolérable
    when: le serveur démarre
    then: il journalise un avertissement énonçant la conséquence fonctionnelle et poursuit
  - given: une variable portant un secret
    when: elle est journalisée, exposée par une réponse d'erreur ou par la référence de configuration
    then: sa valeur n'apparaît jamais, seul son nom est cité
depends_on: [REQ-OPS-001, REQ-SEC-004]
---
```

```yaml
---
id: REQ-OPS-005
title: Sonde de disponibilité distincte de la vivacité
domain: ops
status: draft
criticality: medium
layer: [api]
e2e: n-a
oracle: design
rationale: >
  L'endpoint de santé prouve que le processus répond, pas que le service est utilisable : une base
  injoignable le laisse vert. Un orchestrateur qui ne distingue pas « vivant » de « prêt » route du
  trafic vers une instance incapable de le servir, ou redémarre en boucle une instance saine.
acceptance:
  - given: la base de données accessible
    when: la sonde de disponibilité est interrogée
    then: la réponse indique un service prêt
  - given: la base de données injoignable
    when: la sonde de disponibilité est interrogée
    then: elle signale l'indisponibilité par un état d'erreur serveur, tandis que la sonde de
      vivacité continue de répondre favorablement
  - given: une sonde interrogée sans authentification
    when: elle répond
    then: elle ne divulgue ni version, ni nom d'hôte, ni message d'erreur brut de la base
depends_on: [REQ-OPS-001]
---
```

```yaml
---
id: REQ-OPS-006
title: Arrêt propre sur signal d'extinction
domain: ops
status: draft
criticality: medium
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Sous orchestrateur, une extinction commence par un signal poli puis se termine par une mise à mort.
  Un serveur qui ignore le premier coupe des requêtes en vol : une mutation acquittée côté client
  peut n'être jamais validée, et l'ordonnanceur de rappels peut être interrompu en plein envoi.
acceptance:
  - given: des requêtes en cours de traitement
    when: le serveur reçoit le signal d'extinction
    then: il cesse d'accepter de nouvelles connexions et laisse les requêtes en cours s'achever
      avant de rendre la main
  - given: une requête qui excède le délai de grâce
    when: ce délai est écoulé
    then: le serveur quitte malgré tout, en code de sortie de succès, après l'avoir signalé
  - given: une extinction demandée
    when: elle s'achève
    then: les connexions à la base sont refermées explicitement
depends_on: [REQ-OPS-002]
---
```

```yaml
---
id: REQ-OPS-007
title: Image conteneur et composition de déploiement
domain: ops
status: draft
criticality: high
layer: [api, ui]
e2e: n-a
oracle: design
rationale: >
  L'application d'origine se déploie par une image conteneur ; sans équivalent, la promesse
  d'auto-hébergement reste théorique et l'installation se réduit à « compiler le dépôt soi-même ».
acceptance:
  - given: le dépôt
    when: l'image est construite
    then: elle embarque le serveur et l'interface compilée, s'exécute sous un compte non privilégié
      et ne contient ni secret, ni chaîne de compilation, ni sources
  - given: une machine disposant seulement d'un moteur de conteneurs
    when: la composition de déploiement fournie est démarrée
    then: la base est provisionnée, les migrations appliquées et l'application joignable, sans autre
      geste manuel que le renseignement des variables obligatoires
  - given: la composition
    when: elle est démarrée
    then: le conteneur applicatif déclare un contrôle de santé fondé sur la sonde de disponibilité,
      et n'est considéré prêt qu'une fois la base saine
  - given: une image publiée
    when: elle est démarrée sans volume préexistant puis redémarrée
    then: les données applicatives survivent au redémarrage
depends_on: [REQ-OPS-003, REQ-OPS-004, REQ-OPS-005]
---
```

```yaml
---
id: REQ-OPS-008
title: Publication de versions
domain: ops
status: draft
criticality: medium
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Sans version publiée ni journal des changements, un utilisateur ne peut ni installer un état connu,
  ni évaluer ce qu'une mise à jour lui apporte, ni signaler un défaut de façon exploitable.
acceptance:
  - given: une version marquée dans le dépôt
    when: la chaîne de publication s'exécute
    then: une image conteneur portant cette version et un journal des changements sont publiés
  - given: une instance en service
    when: sa version est interrogée
    then: elle rapporte la version publiée qu'elle exécute
  - given: un état dont une porte de qualité est rouge
    when: une publication est tentée
    then: elle échoue sans rien publier
depends_on: [REQ-OPS-007]
---
```

```yaml
---
id: REQ-OPS-009
title: Sauvegarde et restauration vérifiées
domain: ops
status: draft
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  L'auto-hébergement transfère la responsabilité des données à l'utilisateur. Une procédure de
  restauration jamais rejouée équivaut à une absence de sauvegarde — et le chiffrement au repos des
  secrets ajoute un piège : une archive restaurée sans sa clé est irrécupérable en silence.
acceptance:
  - given: une instance en service
    when: la procédure de sauvegarde documentée est suivie
    then: elle produit une archive cohérente de l'ensemble des données applicatives
  - given: une archive de sauvegarde et une instance vierge
    when: la procédure de restauration documentée est suivie
    then: l'application redémarre sur les mêmes données, et la procédure est rejouée par une
      vérification automatique plutôt que seulement décrite
  - given: une restauration menée avec une clé de chiffrement différente de celle d'origine
    when: un secret chiffré au repos est lu
    then: l'échec est explicite et attribué à la clé, plutôt que silencieux ou confondu avec une
      corruption de données
depends_on: [REQ-OPS-004, REQ-SEC-004]
---
```

```yaml
---
id: REQ-OPS-010
title: Paquets système pour distributions Linux
domain: ops
status: draft
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Le conteneur ne couvre pas tous les auto-hébergeurs : beaucoup administrent un serveur ordinaire et
  attendent une installation par le gestionnaire de paquets, avec un service géré par l'init du
  système, une configuration dans /etc et des mises à jour par la voie habituelle. Sans paquet natif,
  ces utilisateurs sont renvoyés à une installation manuelle non reproductible et non désinstallable.
acceptance:
  - given: le dépôt et une version donnée
    when: les paquets sont construits
    then: un paquet au format Debian et un paquet au format RPM sont produits pour la même version,
      chacun installable sur une distribution cible de cette famille sans dépendance non déclarée
  - given: un paquet installé
    when: l'installation s'achève
    then: le binaire, l'interface compilée, une unité de service et une configuration par défaut sont
      posés aux emplacements standards de la distribution, et le service s'exécute sous un compte
      système dédié, non privilégié et sans session interactive
  - given: le fichier de configuration porteur de secrets
    when: il est posé par le paquet
    then: il n'est lisible que par le compte de service, et ne contient aucun secret prédéfini —
      une installation ne doit jamais aboutir à une clé de chiffrement connue de tous
  - given: une configuration modifiée par l'exploitant
    when: le paquet est mis à jour vers une version ultérieure
    then: ses modifications sont préservées, le service redémarre sur la nouvelle version et les
      migrations sont appliquées
  - given: un paquet désinstallé
    when: la désinstallation ordinaire a lieu
    then: les données et la configuration de l'exploitant sont conservées ; leur suppression exige
      une purge explicitement demandée
  - given: le serveur de base de données
    when: le paquet est installé
    then: il n'est ni imposé ni installé d'office sur la même machine, la connexion restant décrite
      par la configuration
depends_on: [REQ-OPS-003, REQ-OPS-004, REQ-OPS-006]
---
```

```yaml
---
id: REQ-OPS-011
title: Archives binaires autonomes et canaux de distribution additionnels
domain: ops
status: draft
criticality: medium
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Les familles Debian et RPM ne couvrent ni Alpine, ni Arch, ni les gestionnaires transverses. Plutôt
  que de multiplier des recettes maintenues à la main — chacune vieillissant en silence —, la
  distribution repose sur une archive binaire autonome, socle unique dont tout autre canal dérive.
acceptance:
  - given: une version publiée
    when: les artefacts sont construits
    then: une archive autonome par plateforme prise en charge est produite, contenant le binaire du
      serveur, l'interface compilée et la documentation minimale d'exploitation, et fonctionnant par
      simple décompression sans compilation
  - given: une archive autonome
    when: elle est exécutée sur une distribution dépourvue des bibliothèques du système de
      construction
    then: elle démarre néanmoins, la portée exacte des plateformes prises en charge étant documentée
  - given: un canal de distribution additionnel, communautaire ou non
    when: il est ajouté
    then: il consomme les artefacts publiés sans modifier la chaîne de construction, et son caractère
      non officiellement maintenu, le cas échéant, est déclaré
depends_on: [REQ-OPS-003, REQ-OPS-008]
---
```

```yaml
---
id: REQ-OPS-012
title: Intégrité et authenticité des artefacts publiés
domain: ops
status: draft
criticality: high
layer: [api]
e2e: n-a
oracle: design
rationale: >
  Un artefact d'installation est le vecteur de compromission le plus direct d'une application
  auto-hébergée : l'utilisateur exécute avec privilèges ce qu'il vient de télécharger. Distribuer des
  paquets sans moyen d'en vérifier l'origine annulerait le soin porté au reste de la sécurité.
acceptance:
  - given: l'ensemble des artefacts d'une version — image, paquets, archives
    when: ils sont publiés
    then: chacun est accompagné d'une empreinte et d'une signature vérifiables publiquement, et la
      procédure de vérification est documentée à l'endroit où l'utilisateur télécharge
  - given: un artefact altéré après publication
    when: la vérification est menée selon la procédure documentée
    then: elle échoue
  - given: la chaîne de publication
    when: elle s'exécute
    then: aucune clé privée de signature n'apparaît dans les artefacts, les journaux ou le dépôt
depends_on: [REQ-OPS-008, REQ-OPS-010, REQ-OPS-011]
---
```
