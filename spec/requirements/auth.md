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
status: verified
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
title: Jeton d'appareil (API porteur, révocable)
domain: auth
status: verified
criticality: high
layer: [api, ui]
e2e: required
oracle: design
rationale: >
  Un client d'API (hors navigateur) ne peut pas s'appuyer sur un cookie de session ; chaque appareil
  obtient un jeton porteur propre, révocable indépendamment. (Re-cadré : les coquilles natives sont
  hors périmètre, OQ-009/OQ-011 ; le stockage `secureStore` natif est retiré. Voir ADR 0028.)
acceptance:
  - given: une authentification via l'API de session d'appareil (createDeviceSession)
    when: elle réussit
    then: un jeton porteur propre à l'appareil est émis, associé à un libellé et à une date de dernière activité
  - given: un jeton d'appareil
    when: il est présenté en en-tête `Authorization: Bearer`
    then: il authentifie la requête sans cookie et reste révocable indépendamment (REQ-AUT-006)
depends_on: [REQ-AUT-002]
---
```

```yaml
---
id: REQ-AUT-006
title: Liste et révocation des appareils
domain: auth
status: verified
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
status: verified
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
status: verified
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
status: verified
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

```yaml
---
id: REQ-AUT-010
title: Réinitialisation du mot de passe oubliée
domain: auth
status: draft
criticality: high
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Sans ce parcours, un utilisateur qui oublie son mot de passe est enfermé dehors : son compte
  existe, ses données aussi, et rien ne lui permet d'y revenir. C'est le manque le plus grave du
  périmètre de parité, et il le serait même sans exigence de parité.
acceptance:
  - given: une adresse soumise depuis le formulaire de mot de passe oublié
    when: elle correspond à un compte
    then: tout jeton antérieur pour cette adresse est révoqué, un nouveau jeton aléatoire est émis,
      et un message contenant le lien de réinitialisation est mis en file pour envoi
  - given: une adresse qui ne correspond à aucun compte
    when: elle est soumise
    then: la réponse est **identique** au cas précédent — aucune divergence de message, de code ni
      de délai ne doit permettre de savoir si un compte existe
  - given: un jeton de réinitialisation
    when: il est présenté plus d'une heure après son émission
    then: il est refusé
  - given: un jeton valide et un nouveau mot de passe conforme à la politique
    when: la réinitialisation est confirmée
    then: le mot de passe est remplacé, le jeton est consommé et ne peut plus servir, et les
      sessions ouvertes du compte sont invalidées
  - given: des jetons expirés
    when: l'entretien périodique s'exécute
    then: ils sont supprimés
depends_on: [REQ-AUT-002, REQ-AUT-003, REQ-NOT-003]
---
```

```yaml
---
id: REQ-AUT-011
title: Vérification de l'adresse e-mail
domain: auth
status: draft
criticality: medium
layer: [api, ui]
e2e: required
oracle: legacy
rationale: >
  Une adresse non vérifiée rend inopérants les parcours qui en dépendent — à commencer par la
  réinitialisation du mot de passe, qui deviendrait un moyen d'accès pour qui a saisi l'adresse d'un
  autre. La vérification est ce qui rend l'adresse digne de confiance.
acceptance:
  - given: une inscription
    when: elle est enregistrée
    then: un jeton de vérification est émis et un message contenant le lien est mis en file
  - given: un lien de vérification valide
    when: il est suivi
    then: l'adresse est marquée vérifiée et le jeton est consommé
  - given: un lien invalide, déjà consommé ou inconnu
    when: il est suivi
    then: la vérification échoue sans indiquer laquelle des trois causes s'applique
  - given: l'exigence de vérification activée au niveau de l'instance
    when: un compte non vérifié tente de se connecter
    then: la connexion est refusée et la cause en est explicitée à l'utilisateur
  - given: l'exigence de vérification désactivée
    when: un compte non vérifié se connecte
    then: la connexion aboutit — la vérification reste enregistrée mais n'est pas bloquante
depends_on: [REQ-AUT-001, REQ-NOT-003]
---
```

```yaml
---
id: REQ-AUT-012
title: Double authentification par code temporel
domain: auth
status: draft
criticality: high
layer: [core, api, ui]
e2e: required
oracle: legacy
rationale: >
  Un gestionnaire d'abonnements expose des habitudes de consommation et des moyens de paiement. Le
  mot de passe seul ne suffit pas à qui héberge son instance sur l'internet public.
acceptance:
  - given: un compte sans double authentification
    when: l'utilisateur l'active
    then: un secret est généré et présenté sous une forme utilisable par une application
      d'authentification, et l'activation n'est effective **qu'après** vérification d'un code valide
  - given: l'activation confirmée
    when: elle s'achève
    then: un lot de codes de secours à usage unique est remis à l'utilisateur, et une fois seulement
  - given: un compte protégé par double authentification
    when: le mot de passe est validé
    then: la session n'est ouverte qu'après présentation d'un code temporel valide ou d'un code de
      secours, et l'identifiant de session est renouvelé à cet instant
  - given: un code de secours
    when: il a servi
    then: il ne peut plus servir
  - given: un code temporel légèrement décalé dans le temps
    when: il est présenté
    then: il est accepté dans la tolérance capturée sur l'application d'origine, et refusé au-delà
  - given: la désactivation demandée par l'utilisateur
    when: elle est confirmée
    then: le secret et les codes de secours sont détruits
depends_on: [REQ-AUT-002, REQ-AUT-004]
---
```
