# Domaine AUT — Authentification et comptes

> Ce domaine est majoritairement `oracle: design` : le modèle multi-utilisateur est une
> décision de conception (H4), pas une reprise du comportement de l'application d'origine.

```yaml
---
id: REQ-AUT-001
title: Création d'un compte utilisateur
domain: auth
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  Point d'entrée du produit multi-utilisateur ; conditionne toute l'isolation des données.
acceptance:
  - given: une adresse e-mail non encore enregistrée et un mot de passe conforme
    when: l'utilisateur soumet le formulaire d'inscription
    then: le compte est créé, le mot de passe est stocké haché en argon2id et jamais en clair
  - given: une adresse e-mail déjà enregistrée
    when: l'utilisateur soumet le formulaire d'inscription
    then: la réponse est identique au cas nominal et aucune information n'indique l'existence du compte
depends_on: []
---
```

```yaml
---
id: REQ-AUT-002
title: Authentification par e-mail et mot de passe
domain: auth
status: verified
criticality: high
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  Mécanisme d'accès principal sur les trois modalités.
acceptance:
  - given: un compte existant et un mot de passe correct
    when: l'utilisateur s'authentifie
    then: une session est ouverte et l'utilisateur accède à ses données
  - given: un mot de passe incorrect ou un compte inexistant
    when: l'utilisateur s'authentifie
    then: la réponse est 401 avec un message générique identique dans les deux cas
  - given: un mot de passe incorrect
    when: l'utilisateur s'authentifie
    then: le temps de réponse ne permet pas de distinguer un compte existant d'un compte absent
depends_on: [REQ-AUT-001]
---
```

```yaml
---
id: REQ-AUT-003
title: Politique de mot de passe
domain: auth
status: verified
criticality: medium
layer: [core, api, ui]
e2e: required
oracle: design
rationale: >
  Aligné OWASP : longueur minimale plutôt que règles de composition arbitraires.
acceptance:
  - given: un mot de passe de moins de 12 caractères
    when: il est soumis à l'inscription ou au changement
    then: la validation échoue avec un message explicite, côté client ET côté serveur
  - given: un mot de passe figurant dans la liste des mots de passe compromis embarquée
    when: il est soumis
    then: la validation échoue
depends_on: []
---
```

```yaml
---
id: REQ-AUT-004
title: Session web par jeton opaque en cookie
domain: auth
status: accepted
criticality: high
layer: [api]
e2e: required
oracle: design
rationale: >
  Un jeton opaque révocable immédiatement, contrairement à un JWT (décision figée AGENTS.md §9).
acceptance:
  - given: une authentification réussie depuis la modalité web
    when: la réponse est émise
    then: le cookie de session porte HttpOnly, Secure et SameSite=Lax, et ne contient aucune donnée métier
  - given: une session ouverte
    when: elle dépasse sa durée d'inactivité configurée
    then: toute requête ultérieure est rejetée en 401
depends_on: [REQ-AUT-002]
---
```

```yaml
---
id: REQ-AUT-005
title: Jeton d'appareil pour les modalités desktop et mobile
domain: auth
status: accepted
criticality: high
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Les coquilles natives ne peuvent pas s'appuyer sur un cookie de navigateur ; chaque appareil
  doit être révocable indépendamment.
acceptance:
  - given: une authentification depuis une coquille native
    when: elle réussit
    then: un jeton propre à l'appareil est émis, associé à un libellé et à une date de dernière activité
  - given: un jeton d'appareil
    when: il est stocké côté client
    then: il passe exclusivement par PlatformAdapter.secureStore, jamais par localStorage
depends_on: [REQ-AUT-002]
---
```

```yaml
---
id: REQ-AUT-006
title: Liste et révocation des appareils
domain: auth
status: accepted
criticality: high
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Contrepartie indispensable des jetons d'appareil : sans révocation, ils sont un risque net.
acceptance:
  - given: plusieurs appareils appairés
    when: l'utilisateur consulte ses appareils
    then: la liste affiche libellé, plateforme et dernière activité, et distingue l'appareil courant
  - given: un appareil révoqué
    when: il émet une requête avec son ancien jeton
    then: la réponse est 401 immédiatement, sans délai de propagation
depends_on: [REQ-AUT-005]
---
```

```yaml
---
id: REQ-AUT-007
title: Changement de mot de passe
domain: auth
status: accepted
criticality: high
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Un changement de mot de passe doit couper les accès existants, sinon il ne remédie à rien.
acceptance:
  - given: un utilisateur authentifié fournissant son mot de passe actuel correct
    when: il définit un nouveau mot de passe conforme
    then: le hachage est remplacé et toutes les sessions et jetons d'appareil sont invalidés sauf la session courante
  - given: un mot de passe actuel incorrect
    when: le changement est soumis
    then: la réponse est 403 et aucun état n'est modifié
depends_on: [REQ-AUT-003, REQ-AUT-006]
---
```

```yaml
---
id: REQ-AUT-008
title: Limitation du taux de tentatives d'authentification
domain: auth
status: accepted
criticality: high
layer: [api]
e2e: optional
oracle: design
rationale: >
  Sans limitation, la politique de mot de passe ne protège pas contre l'attaque par force brute.
acceptance:
  - given: un nombre de tentatives échouées dépassant le seuil pour un même compte
    when: une nouvelle tentative est émise
    then: la réponse est 429 avec un en-tête Retry-After, y compris si le mot de passe est correct
  - given: le seuil atteint pour une adresse IP sur des comptes différents
    when: une nouvelle tentative est émise
    then: la limitation s'applique également
depends_on: [REQ-AUT-002]
---
```

```yaml
---
id: REQ-AUT-009
title: Déconnexion
domain: auth
status: accepted
criticality: medium
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Doit invalider côté serveur, pas seulement effacer le jeton côté client.
acceptance:
  - given: une session active
    when: l'utilisateur se déconnecte
    then: le jeton est invalidé côté serveur et le cookie est expiré
  - given: une session déjà invalidée
    when: la déconnexion est rejouée
    then: la réponse reste 204 (idempotence)
depends_on: [REQ-AUT-004]
---
```
