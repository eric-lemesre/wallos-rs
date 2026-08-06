# ADR 0042 — Exclusion transverse des agrégats : la règle s'applique par occurrence dans l'échéancier

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-STA-003 (« Exclusion des abonnements non actifs des agrégats »), `oracle: legacy`,
  criticality high, layer `[core, api]`, e2e required, dépend de REQ-SUB-008/009/010.

## Problème

STA-003 n'est pas un nouvel agrégat mais une **règle transverse** : les trois états non actifs —
désactivé (SUB-008), terminé (SUB-009), en essai gratuit (SUB-010) — doivent être exclus de **tout**
agrégat, « selon la règle propre à chaque état » (critère #1), et un abonnement **réactivé** doit y être
**immédiatement réintégré** (critère #2). La question de conception : comment appliquer l'exclusion de
l'essai à un agrégat **temporel** — l'échéancier des prochains paiements (REQ-STA-005) — qui n'est pas
une simple somme « à ce jour » mais une énumération d'occurrences futures ?

## Constat legacy

Wallos 5.4.2 filtre `WHERE inactive = 0` dans **chacune** de ses requêtes de statistiques (coût mensuel,
annuel, répartitions, échéancier). C'est la seule exclusion qu'il connaît (ni date de fin, ni essai). Le
constat est gelé dans `e2e/fixtures/oracles/REQ-STA-003.json`. Les états `terminé` et `essai` sont des
**extensions** du modèle subtrack, à appliquer de façon cohérente au même titre.

## Décision

### La règle propre à chaque état

- **Désactivé** : exclu **intégralement** de tous les agrégats (filtre `active = true` en amont).
- **Terminé** : dans les agrégats « à ce jour » (total, répartition), exclu si `end_date < today` ; dans
  les agrégats temporels (échéancier, évolution), il borne la **fin** de la fenêtre effective.
- **Essai** : dans les agrégats « à ce jour », exclu tant que `today < trial_end_date` ; dans
  l'échéancier, la règle s'applique **par occurrence** — aucun paiement n'est dû pendant l'essai, donc
  toute occurrence **strictement antérieure** à `trial_end_date` est écartée, la borne basse effective
  devenant `max(from, trial_end_date)`. Une occurrence tombant **exactement** sur `trial_end_date` est
  due (l'essai est alors terminé, cohérent avec `is_in_trial` ⇔ `date < trial_end`).

Choix retenu : **exclusion par occurrence**, pas exclusion de l'abonnement entier. Un abonnement en essai
a de **vraies** échéances après la fin d'essai ; les masquer toutes priverait la vue de trésorerie de
paiements légitimes. Symétrique de la borne haute `end_date` déjà portée par `occurrences_in_range`.

### Critère #2 : réintégration immédiate

Aucun cache ni état dérivé : chaque agrégat lit l'état courant (`active`, `end_date`, `trial_end_date`)
au moment du calcul. Repasser `active` à vrai réintègre l'abonnement au prochain appel, sans étape.

## Conséquences

- `core::occurrences_in_range` gagne un paramètre `trial_end: Option<NaiveDate>` (borne basse), passé par
  `server::schedule::get_upcoming_payments` depuis `row.trial_end_date`. `core::billable_amounts` reste la
  primitive des trois exclusions « à ce jour ».
- Annotation `#[requirement(REQ-STA-003)]` posée sur les quatre surfaces d'agrégat (total de liste,
  répartition, évolution, échéancier) + la primitive cœur, matérialisant la nature transverse de la règle.
- Aucune divergence d'affichage nouvelle : le legacy `inactive = 0` est respecté ; les états fin/essai
  sont des extensions cohérentes, déjà couvertes par ADR 0041 (essai) pour les agrégats de coût.
- Tests : cœur (`schedule` bornes basse/haute composées, `stats` règle combinée + réactivation),
  intégration (`aggregates_exclusion.rs`, 4 surfaces), e2e (`aggregates-exclusion.spec.ts`, critère #2).
