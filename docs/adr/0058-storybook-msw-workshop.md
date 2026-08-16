# ADR 0058 — Atelier de composants : Storybook, et MSW à la seule frontière réseau

- **Statut** : accepté (2026-08-16)
- **Contexte** : étape 5 du chantier ouvert par l'ADR 0057. Justification de dépendances au titre de
  **R6**. Boucle retenue par le responsable (2026-08-16) : **Storybook seul**, sans l'étape amont
  « Open Design » qu'utilise `ergonomia` — il n'y a ici aucune équipe métier à convaincre.

## Problème

Le socle visuel est posé (REQ-CLT-008) mais rien ne permet de **voir** un composant. Pour juger d'un
état vide, d'une erreur ou d'un formulaire en cours de saisie, il faut aujourd'hui lancer Postgres,
le serveur, le client, s'authentifier, et **fabriquer la donnée** qui produit l'état voulu. Le coût
est tel que ces états ne sont jamais regardés — ils ne sont que testés, ce qui n'est pas la même
chose : un test dit qu'un texte est présent, il ne dit pas que l'écran est lisible.

## Décision

### Storybook comme atelier

Chaque état d'un composant devient une *story* : une fonction qui le rend avec des props données,
affichée seule. Le catalogue qui en résulte est une **documentation qui ne se périme pas**, puisqu'elle
est le code lui-même exécuté.

Conséquence de méthode reprise d'`ergonomia` : **la story est la maquette**. Aucune maquette HTML
jetable n'est committée — une image ment dès la semaine suivante, une story non.

### MSW à la frontière réseau, et nulle part ailleurs

Le point délicat est le rendu des écrans **réels**. La tentation serait de leur injecter des données
par des props factices, ce qui obligerait à les réécrire pour l'atelier — donc à tester autre chose
que ce qui tourne.

On intercepte donc uniquement la **frontière HTTP** (MSW). Le client généré, l'i18n, les hooks et
toute la logique s'exécutent **inchangés** : ce que montre l'atelier est ce que verra l'utilisateur.
C'est aussi ce qui rend les états *données / vide / erreur* fidèles, chacun n'étant qu'une réponse
différente du même gestionnaire.

### Dépendances (R6)

| Paquet | Rôle | Portée |
|---|---|---|
| `storybook`, `@storybook/react-vite` | l'atelier, sur le même Vite que la coquille | dev |
| `@storybook/addon-docs` | page de documentation générée par composant | dev |
| `msw`, `msw-storybook-addon` | interception réseau, et son branchement dans l'atelier | dev |

Toutes en **dépendances de développement** du seul paquet `@wallos/ui` : aucune n'entre dans le
paquet livré, ni dans la coquille. `@storybook/react-vite` réemploie la configuration Vite existante
plutôt que d'introduire une seconde chaîne de construction.

### Hors du chemin critique de la CI

L'atelier **ne rejoint pas** le job `frontend`, qui garde le pull request court. Il a son propre
workflow, déclenché quand `frontend/ui/**` change et **à la demande**, publiant la galerie en
artefact téléchargeable. Motif emprunté à `ergonomia` : le dépôt est public mais la galerie n'a pas
à être un site — un artefact suffit à la relecture, sans hébergement à maintenir.

## Conséquences

- Couverture livrée volontairement **étroite** : un domaine servant de patron, plutôt qu'un
  saupoudrage sur vingt-deux composants. Les suivants se branchent sur le même modèle, et cette
  bordure est **déclarée** plutôt que laissée à supposer.
- Les primitives du design system (`ds/`) n'existent pas encore : l'atelier ouvre donc sur les
  écrans réels. C'est l'inverse de l'ordre habituel, et c'est assumé — les primitives se dégageront
  de ce que les écrans répètent, plutôt que d'être devinées avant tout usage.
- La **charte d'interaction** reste à écrire, et le restera tant que les primitives manquent : elle
  documente l'existant, et n'aurait aujourd'hui presque rien à décrire au-delà des jetons.
- Coût assumé : l'installation de développement grossit nettement. Aucun effet sur le produit livré
  ni sur la durée du job `frontend`.
