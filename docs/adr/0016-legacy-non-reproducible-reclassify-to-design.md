# ADR 0016 — Exigences `oracle: legacy` non reproductibles : reclassement en `design` tracé

## Contexte

`OQ-007` restait ouverte et concernait le protocole §8.1 : certaines exigences `oracle: legacy`
ne pourront pas être capturées contre l'application d'origine gelée (Wallos 5.4.2, ADR 0011) —
comportement non déterministe, fonctionnalité absente, ou dépendance à un service tiers. Sans règle
explicite, l'agent risquerait d'« inventer » une règle métier plausible mais fausse, ce que tout le
dispositif d'oracle cherche à empêcher.

Options recensées : A) les basculer en `oracle: design` avec une décision explicite — B) les exclure
du périmètre. Recommandation agent : A, avec ADR obligatoire.

## Décision

Le responsable du dépôt a arbitré **l'option A**.

Lorsqu'une exigence `oracle: legacy` s'avère non reproductible contre la cible legacy figée, elle est
**reclassée en `oracle: design`** au moyen d'un **ADR dédié** qui documente :

1. la raison précise de la non-reproductibilité (non-déterminisme / fonctionnalité absente /
   dépendance tierce) ;
2. la règle de `design` retenue à la place, et son fondement ;
3. la portée : reclassement **total** ou **partiel** (ex. conserver l'oracle legacy pour les valeurs
   numériques, ne passer en `design` que l'aspect non capturable — cf. tension REQ-SUB-017/STA-004
   signalée par l'ADR 0012).

Le reclassement met à jour le champ `oracle` de l'exigence dans `spec/requirements/*.md` **et**
`spec/requirements.lock.yaml`, avec un lien vers l'ADR. **Aucun basculement silencieux** : une
exigence dont l'oracle change sans ADR est un défaut de revue.

L'exclusion pure et simple (option B) n'est pas retenue par défaut ; elle exigerait elle-même un ADR
justifiant la perte fonctionnelle.

## Conséquences

- Fournit le mécanisme formel attendu par l'ADR 0012 (payeur = membre de foyer vs étiquette legacy).
- Chaque reclassement reste traçable et revu ; l'oracle du projet ne dérive jamais en silence.
- Ce mécanisme s'applique au cas par cas, au moment d'implémenter l'exigence concernée, pas de façon
  anticipée sur tout un domaine.

## Liens

- AGENTS.md §0, §3 (cycle de vie / champ `oracle`), §8.1 ; ADR 0011 (cible legacy), ADR 0012 (foyer).
- `spec/OPEN-QUESTIONS.md` : OQ-007 (résolue par cet ADR).

## Statut

accepted
