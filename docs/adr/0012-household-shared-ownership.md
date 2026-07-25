# ADR 0012 — Foyer partagé comme unité de propriété et d'isolation

## Contexte

`OQ-002` (périmètre du foyer et des payeurs) restait ouverte et bloquait `REQ-SUB-017`
(rattachement à un payeur) et `REQ-STA-004` (répartition par payeur). Sa résolution est
**structurante pour `REQ-SEC-001`** : selon l'option retenue, l'isolation des données se fait par
compte individuel ou par foyer.

Options recensées : A) payeur = étiquette sans compte — B) membres invités avec accès en lecture —
C) foyer partagé avec droits d'écriture. La recommandation agent était A pour la v1 (B et C
multiplient la surface d'autorisation à tester).

## Décision

Le responsable du dépôt a arbitré **l'option C : un foyer**.

- Le **foyer** (`household`) est l'unité de propriété et d'isolation des données métier, **et non**
  le compte individuel. Toute entité métier (abonnement, catégorie, payeur, moyen de paiement…)
  porte un `household_id` non nullable.
- Un foyer regroupe un ou plusieurs comptes utilisateurs, tous disposant de droits de **lecture et
  d'écriture** sur les données du foyer.
- Un **payeur** (`REQ-SUB-017`) est un membre du foyer, pas une simple étiquette.
- Un compte appartient à un foyer (à sa création, un foyer personnel est créé ; l'invitation
  d'autres comptes au foyer est un parcours à spécifier côté `REQ-AUT-*`).

## Conséquences

### Isolation (`REQ-SEC-001`, AGENTS.md §9)

Le garde-fou `Actor` (ADR 0006) porte le **contexte de foyer**. Les repositories filtrent par
`household_id`, jamais par identifiant seul. Les trois tests d'autorisation par `operation_id`
deviennent :

1. accès par un membre du foyer propriétaire ⟶ `2xx` ;
2. accès par un utilisateur authentifié d'un **autre** foyer ⟶ `404` (jamais `403`) ;
3. accès non authentifié ⟶ `401`.

### Tension avec l'oracle legacy — à trancher séparément

`REQ-SUB-017` et `REQ-STA-004` sont `oracle: legacy`. Or dans Wallos 5.4.2 (ADR 0011), le
« payeur » est une **étiquette**, pas un compte membre d'un foyer partagé. Le modèle C **diverge**
donc du comportement de référence sur l'aspect « compte/membre ».

Conséquence : il faudra, via le mécanisme d'`OQ-007` (ADR obligatoire), soit
- conserver l'**oracle legacy pour les valeurs numériques** (totaux de la répartition par payeur,
  `REQ-STA-004`) tout en traitant l'aspect « payeur = membre de foyer » en `oracle: design`, soit
- reclasser explicitement la partie « foyer » de ces exigences en `design`.

Cet ADR **n'inclut pas** cette reclassification : elle relève d'une décision distincte prise au
moment d'implémenter `REQ-SUB-017`/`REQ-STA-004`. Le présent ADR ne fait que fixer le modèle de
propriété ; il signale la tension sans la contourner (AGENTS.md §0).

### Surface d'autorisation

L'option C élargit la surface de test d'autorisation (appartenance au foyer, invitations,
révocation d'appartenance). Ce coût est assumé par la décision.

## Liens

- AGENTS.md §0, §9 ; ADR 0006 (garde-fou `Actor`), ADR 0011 (cible legacy).
- `spec/OPEN-QUESTIONS.md` : OQ-002 (résolue par cet ADR), OQ-007 (mécanisme de reclassification).
- Exigences concernées : REQ-SEC-001, REQ-SUB-017, REQ-STA-004, REQ-AUT-001.

## Statut

accepted
