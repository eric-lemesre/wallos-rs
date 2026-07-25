# ADR 0007 — Interdiction des mentions `Co-authored-by` dans les commits générés par agent

## Contexte

Les agents IA qui contribuent à ce dépôt produisent des commits. Par défaut, certains outils ou
workflows d'agent peuvent injecter une ligne `Co-authored-by` dans le message de commit pour
attribuer la contribution à l'agent. Cette pratique crée une ambiguïté juridique et une confusion
sur la responsabilité finale du code : le dépôt est sous licence AGPL-3.0-or-later et l'auteur
humain reste responsable des modifications commitées.

## Décision

Tout commit créé par un agent IA sur ce dépôt doit respecter les règles suivantes :

- Le message de commit ne contient **aucune** ligne `Co-authored-by`.
- Le message de commit ne contient **aucun** mécanisme équivalent d'attribution tierce
  (`Co-Authored-By`, `Signed-off-by` d'un bot, etc.).
- L'auteur du commit (`git config user.name` / `user.email`) reste l'humain responsable du dépôt.
- Un commit par phase ou par exigence reste la granularité cible.

## Conséquences

- La revue des commits via `git log` doit permettre de vérifier cette règle.
- Les agents doivent s'assurer de configurer `user.name` et `user.email` avant de commiter si ce
  n'est pas déjà fait, ou utiliser les valeurs existantes du dépôt.
- Une mention accidentelle `Co-authored-by` est traitée comme une violation bloquante : le commit
  doit être amendé ou refait.

## Liens

- AGENTS.md §0 (R0).

## Statut

accepted
