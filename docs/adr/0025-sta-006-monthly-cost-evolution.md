# ADR 0025 — Évolution du coût mensuel : sémantique « actif à ce mois-là » (oracle de conception)

- **Statut** : accepté (2026-07-31)
- **Contexte** : REQ-STA-006 (évolution du coût sur douze mois), exigence `oracle: design`,
  criticality low, layer `[core, api, ui]`, dépend de REQ-STA-001.

## Problème

L'acceptance STA-006 impose une contrainte précise : « chaque point reflète les abonnements **actifs
à cette date**, pas l'état courant projeté ». Le piège à éviter est une série **plate** — prendre le
total mensuel courant et le répéter sur douze mois. La série doit au contraire **varier** selon qui
était réellement actif chaque mois.

Le modèle de données ne conserve pas d'historique événementiel (pas de journal d'activation) : on
dispose, par abonnement, de `first_payment` (début), `end_date` (fin programmée éventuelle, REQ-SUB-009),
du drapeau `active` (désactivation manuelle courante, REQ-SUB-008), du cycle et du montant. Il faut
donc **définir** ce que « actif à un mois donné » signifie à partir de ces seules données — d'où
l'oracle **de conception** (aucun comportement legacy Wallos à capturer : son graphe d'historique
repose sur une table de paiements que subtrack ne modélise pas).

## Décision

**Un abonnement contribue au coût d'un mois M si sa fenêtre d'activité `[first_payment, end_date]`
recoupe le mois** : `first_payment <= dernier jour de M` **ET** (`end_date` absente **OU** `end_date >=
premier jour de M`). Le coût porté est son **coût mensuel normalisé** (REQ-STA-001) converti dans la
devise cible.

Conséquences concrètes de cette définition :

1. **Historique via les dates de début/fin.** Un abonnement créé récemment n'apparaît que sur les
   derniers points ; un abonnement terminé disparaît des mois postérieurs à sa date de fin. La série
   n'est jamais l'état courant aplati.
2. **Désactivation manuelle = exclusion totale.** Un abonnement `active = false` est **exclu de toute
   la série** (comme il l'est du total courant, REQ-SUB-008) : faute de savoir *quand* il a été
   désactivé, on ne peut pas reconstruire ses fenêtres d'activité passées — mieux vaut l'exclure que
   d'inventer un historique. C'est une limitation assumée de la conception, cohérente avec le reste.
3. **Devise de référence + arrondi d'affichage.** Chaque coût mensuel est converti dans la devise de
   référence du foyer (REQ-CUR-001/003 ; override `?currency=` possible). Un abonnement au coût non
   convertible (taux manquant) est **exclu** (jamais compté zéro, revue STA-001 F2). Le total de chaque
   point est arrondi puis **formaté aux décimales de la devise** (REQ-CUR-005/007) pour un axe cohérent
   (« 0.00 » comme « 42.50 », jamais « 0 » nu).
4. **Fenêtre paramétrable, bornée.** `?months=N` (défaut 12), série du plus ancien au plus récent
   s'achevant au **mois courant**. `N` hors `1..=60` → 422 (garde-fou).

## Découpage core / serveur

- **`core::monthly_cost_evolution(spans, anchor, months)`** : fonction **pure** (REQ-STA-008, aucune
  horloge) qui ne fait que le **fenêtrage temporel** sur des `CostSpan { start, end, monthly }` dont le
  coût est **déjà normalisé et exprimé dans une devise commune**. Le domaine reste sans I/O ni table de
  taux, et l'intersection fenêtre/mois y est unitairement testée (début récent, fin dépassée, cumul,
  série vide).
- **`server::statistics::get_cost_evolution`** : résout la devise cible, liste les abonnements **actifs**
  du foyer (§9), convertit chaque coût mensuel (taux chargés à la demande), ancre `anchor` sur l'horloge
  serveur (seul point d'injection temporelle), puis arrondit/format les totaux.

## Conséquences

- **Nouvelle opération** `getCostEvolution` (`GET /statistics/cost-evolution`) dans un nouveau module
  serveur `statistics` ; couverte par les trois tests authz owner/other/anon (isolation §9).
- **e2e** : `e2e: optional` ; un spec `@design` léger vérifie l'affichage des douze points. Le
  comportement fin (fenêtrage, conversion, exclusions) est couvert au niveau core (unitaire) + api
  (intégration, dont conversion USD→EUR et exclusion des inactifs).
- **Évolutions futures** : si un journal d'activation était introduit (historisation des toggles
  `active`), la règle #2 pourrait être raffinée sans changer la signature du domaine (les `CostSpan`
  deviendraient plus fins).
