# Domaine CLT — Clients

> Le produit vise **trois clients** : web, bureau et mobile (OQ-014, réouverture d'OQ-009,
> ADR 0055). C'est une **divergence assumée** avec l'application d'origine, qui n'en a qu'un.
> La parité continue de régir le **comportement métier** — les oracles `legacy` restent la référence
> — mais elle ne commande plus le périmètre des modalités.
>
> Règle directrice : **une seule interface**. Les coquilles natives empaquettent l'interface web
> existante ; elles n'en sont pas une réécriture. Tout ce qui les distingue passe par un adaptateur
> de plateforme unique (REQ-CLT-003), afin que le code d'interface ignore sur quoi il s'exécute.
>
> Corollaire : **un seul dépôt**. Coquilles et recettes d'empaquetage vivent dans `wallos-rs`
> (règle R9, ADR 0056) — sans quoi la traçabilité (R1), la porte de dérive du contrat d'API (R8) et
> la version commune exigée par REQ-CLT-007 perdraient leur objet. Un dépôt unique ne veut pas dire
> un artefact unique : serveur et clients se construisent et se publient séparément.

```yaml
---
id: REQ-CLT-003
title: Adaptateur de plateforme
domain: clients
status: draft
criticality: high
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Sans point de passage unique, chaque capacité native se paie d'une condition dispersée dans
  l'interface, et le web finit par régresser à chaque ajout. L'adaptateur est ce qui permet à trois
  coquilles de partager une seule interface plutôt que d'en faire diverger trois copies.
acceptance:
  - given: le code de l'interface
    when: il utilise une capacité dépendante de la plateforme — stockage sécurisé, notification
      système, ouverture d'un lien externe
    then: il passe par l'adaptateur, sans jamais tester la plateforme sur laquelle il s'exécute,
      ce qu'une porte automatique vérifie
  - given: une exécution dans un navigateur ordinaire
    when: l'adaptateur est sollicité
    then: une implémentation web par défaut répond, de sorte que le client web reste complet
  - given: une capacité absente ou refusée sur la plateforme courante
    when: elle est sollicitée
    then: l'adaptateur le signale explicitement à l'appelant, qui dégrade la fonctionnalité sans
      interrompre l'application
depends_on: []
---
```

```yaml
---
id: REQ-CLT-005
title: Instance auto-hébergée configurable
domain: clients
status: draft
criticality: high
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Un client natif n'a pas d'origine implicite : contrairement au web, rien ne lui indique à quel
  serveur parler. Sans saisie d'instance, un client d'application auto-hébergée est inutilisable.
acceptance:
  - given: un client natif lancé pour la première fois
    when: aucune instance n'est enregistrée
    then: il demande l'adresse de l'instance avant toute autre chose, et vérifie qu'elle répond
      et qu'elle est compatible avant de l'accepter
  - given: une adresse d'instance en clair
    when: elle est saisie et ne désigne pas la machine locale
    then: elle est refusée ou exige une confirmation explicite, les jetons d'authentification ne
      devant pas circuler en clair sur un réseau
  - given: une instance enregistrée
    when: l'utilisateur en désigne une autre
    then: le jeton et les données locales rattachés à la précédente sont effacés, sans quoi un
      changement d'instance laisserait des données d'un autre foyer sur l'appareil
  - given: une instance devenue injoignable
    when: le client démarre
    then: il l'indique et propose de réessayer ou de changer d'instance, plutôt que d'échouer sans
      explication
depends_on: [REQ-CLT-003, REQ-AUT-005]
---
```

```yaml
---
id: REQ-CLT-004
title: Stockage sécurisé du jeton sur coquille native
domain: clients
status: draft
criticality: high
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Sur un poste ou un téléphone, un jeton d'accès durable conservé dans le stockage web est lisible
  par tout ce qui accède au profil de l'application. Le système d'exploitation offre un magasin
  prévu pour cela ; ne pas l'utiliser reviendrait à annuler le bénéfice du jeton révocable.
acceptance:
  - given: une authentification réussie sur une coquille native
    when: le jeton est conservé
    then: il l'est dans le magasin de secrets du système, et jamais dans le stockage du moteur web
  - given: une déconnexion, ou la révocation du jeton côté serveur
    when: elle survient
    then: le secret est effacé du magasin du système
  - given: un magasin de secrets indisponible ou refusé par l'utilisateur
    when: le jeton doit être conservé
    then: l'application le dit et fonctionne en session non persistante, sans se rabattre
      silencieusement sur un stockage moins sûr
  - given: un journal, un rapport d'erreur ou une trace de diagnostic
    when: il est produit par une coquille native
    then: le jeton n'y figure jamais
depends_on: [REQ-CLT-003, REQ-AUT-005]
---
```

```yaml
---
id: REQ-CLT-001
title: Coquille de bureau
domain: clients
status: draft
criticality: high
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Un suivi d'abonnements se consulte au fil de l'eau : une application lancée depuis le bureau, qui
  se souvient de sa fenêtre et n'exige pas d'ouvrir un navigateur sur la bonne adresse, change
  l'usage quotidien plus qu'une fonctionnalité supplémentaire.
acceptance:
  - given: les plateformes de bureau prises en charge — Linux, macOS et Windows
    when: la coquille est construite
    then: elle empaquette l'interface web existante, sans seconde implémentation de l'interface
  - given: la coquille lancée
    when: l'utilisateur ferme puis rouvre l'application
    then: sa session et l'instance configurée sont retrouvées, et les dimensions de la fenêtre
      restaurées
  - given: un lien vers un site externe dans l'interface
    when: il est activé depuis la coquille
    then: il s'ouvre dans le navigateur du système, jamais dans la fenêtre de l'application
  - given: une nouvelle version de l'interface livrée par le serveur
    when: la coquille se connecte à une instance plus récente qu'elle
    then: l'incompatibilité est détectée et signalée, plutôt que produite sous forme d'erreurs
      inexplicables
depends_on: [REQ-CLT-003, REQ-CLT-004, REQ-CLT-005]
---
```

```yaml
---
id: REQ-CLT-002
title: Coquille mobile
domain: clients
status: draft
criticality: medium
layer: [ui]
e2e: optional
oracle: design
rationale: >
  Le rappel avant échéance n'a de valeur que là où l'utilisateur le lit. Le téléphone est la surface
  naturelle d'une notification, et la PWA ne la couvre pas de façon fiable sur toutes les plateformes.
acceptance:
  - given: les plateformes mobiles prises en charge — Android et iOS
    when: la coquille est construite
    then: elle empaquette la même interface, sans seconde implémentation
  - given: une instance auto-hébergée accessible sur le réseau local
    when: le client mobile s'y connecte
    then: la connexion aboutit, y compris avec un certificat propre au réseau de l'utilisateur, dès
      lors que celui-ci l'a explicitement accepté
  - given: le geste de retour du système
    when: il est utilisé
    then: il remonte la navigation de l'interface et ne quitte l'application qu'à la racine
  - given: un appareil dont l'affichage comporte des zones réservées au système
    when: l'interface est rendue
    then: aucun élément interactif ne se trouve masqué par ces zones
depends_on: [REQ-CLT-003, REQ-CLT-004, REQ-CLT-005]
---
```

```yaml
---
id: REQ-CLT-006
title: Confinement des capacités de la coquille native
domain: clients
status: draft
criticality: high
layer: [ui]
e2e: n-a
oracle: design
rationale: >
  Une coquille native transforme une page web en programme local : une faille d'interface qui ne
  coûtait qu'un onglet peut désormais coûter l'accès au disque de l'utilisateur. Le confinement des
  capacités est ce qui distingue une application empaquetée d'un navigateur sans garde-fou.
acceptance:
  - given: les capacités offertes par la coquille au contenu web
    when: elles sont déclarées
    then: seules celles effectivement requises par une exigence le sont, la position par défaut
      étant le refus
  - given: un contenu distant
    when: il est chargé par la coquille
    then: seule l'instance configurée par l'utilisateur peut l'être, toute autre origine étant
      refusée
  - given: un artefact de publication
    when: il est construit
    then: les outils de développement et l'inspection à distance y sont désactivés
  - given: une exigence retirée ou une capacité devenue inutile
    when: la revue a lieu
    then: la déclaration de capacités est révisée en conséquence, une porte automatique signalant
      toute capacité déclarée sans exigence qui la justifie
depends_on: [REQ-CLT-001]
---
```

```yaml
---
id: REQ-CLT-007
title: Artefacts d'installation des clients
domain: clients
status: draft
criticality: medium
layer: [ui]
e2e: n-a
oracle: design
rationale: >
  Un client natif que l'utilisateur doit compiler n'est pas un client natif. Les formats attendus
  diffèrent de ceux du serveur : une application de bureau s'installe avec son entrée de menu et ses
  icônes, pas avec une unité de service.
acceptance:
  - given: une version publiée
    when: les artefacts des clients sont construits
    then: chaque plateforme de bureau prise en charge reçoit son format d'installation habituel, et
      chaque plateforme mobile le sien, tous portant la même version que le serveur
  - given: un paquet de bureau pour une distribution Linux
    when: il est installé
    then: l'application apparaît dans le menu du système avec ses icônes, et se désinstalle
      proprement — sans unité de service ni compte système, à la différence du paquet serveur
  - given: une plateforme pour laquelle aucun certificat de signature n'est disponible
    when: son artefact est publié
    then: son absence de signature est déclarée là où l'utilisateur télécharge, plutôt que découverte
      au moment de l'avertissement du système
  - given: un client et un serveur de versions différentes
    when: ils dialoguent
    then: la compatibilité est vérifiée et l'incompatibilité signalée à l'utilisateur
depends_on: [REQ-CLT-001, REQ-CLT-002, REQ-OPS-011, REQ-OPS-012]
---
```
