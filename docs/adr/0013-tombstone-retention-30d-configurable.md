# ADR 0013 — Rétention des pierres tombales : 30 jours, paramétrable côté serveur

## Contexte

`OQ-004` restait ouverte et bloquait `REQ-SYN-002` (pierres tombales). La durée de rétention des
tombstones détermine la fenêtre maximale pendant laquelle un appareil peut rester hors ligne avant
d'être contraint à une **resynchronisation complète** : au-delà, ses tombstones peuvent avoir été
purgés côté serveur, et une synchronisation incrémentale par curseur `since` (H5, `REQ-SYN-003`)
ne pourrait plus lui transmettre les suppressions manquées.

Options recensées : A) 30 jours — B) 90 jours — C) rétention illimitée. La recommandation agent
était B.

## Décision

Le responsable du dépôt a arbitré : **30 jours par défaut, valeur paramétrable côté serveur**.

- La rétention par défaut des pierres tombales est de **30 jours**.
- La valeur est **configurable par l'opérateur du serveur** (variable d'environnement /
  configuration serveur), jamais par l'utilisateur final ni par le client.
- Un appareil dont le dernier curseur `since` est plus ancien que la fenêtre de rétention effective
  reçoit une réponse explicite de **resynchronisation complète requise** (jamais une synchronisation
  incrémentale silencieusement incomplète).

## Conséquences

- `REQ-SYN-002` (pierres tombales) et `REQ-SYN-003` (récupération incrémentale par curseur) peuvent
  avancer : le serveur purge les tombstones plus vieux que la fenêtre configurée, et signale la
  péremption du curseur.
- La valeur de rétention vit dans la configuration serveur (à côté du reste de la configuration de
  `crates/server`), avec 30 jours comme défaut sûr. Elle n'est pas un secret : ADR 0013 ≠ REQ-SEC-004.
- Contrainte de test : la logique de péremption du curseur doit être testée avec une fenêtre
  **injectée** (pas l'horloge système ; cf. REQ-STA-008 / porte `lint-clock`), pour rester
  reproductible.
- Choix distinct de la recommandation agent (B = 90 j) : assumé par la décision, et de toute façon
  ajustable par l'opérateur sans changement de code.

## Liens

- AGENTS.md §0, §9 ; H5 (curseur `since`) ; REQ-STA-008 (déterminisme, date en paramètre).
- `spec/OPEN-QUESTIONS.md` : OQ-004 (résolue par cet ADR).
- Exigences concernées : REQ-SYN-002, REQ-SYN-003.

## Statut

accepted
