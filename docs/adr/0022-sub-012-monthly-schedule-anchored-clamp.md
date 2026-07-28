# ADR 0022 — Échéance mensuelle : ancrage + clamp (override délibéré du bug PHP de Wallos)

- **Statut** : accepté (2026-07-28)
- **Contexte** : REQ-SUB-012 (calcul de la prochaine échéance mensuelle), exigence *pilote de l'oracle*.

## Problème

REQ-SUB-012 est marquée `oracle: legacy` (« le comportement des fins de mois doit être **capturé**,
jamais déduit »). La **capture** du comportement réel de Wallos 5.4.2
(`endpoints/cronjobs/updatenextpayment.php`) révèle qu'il **contredit** le texte d'acceptation de la
propre spec :

- **Wallos (capturé, exécuté)** : ajoute `DateInterval('P{frequency}M')` **à la date d'échéance
  courante** (ni ancrage sur le jour d'origine, ni clamp). PHP **déborde** les fins de mois :
  - `2025-01-31 + P1M → 2025-03-03` (saute février), puis `→ 04-03 → 05-03` (se fixe sur le 3) ;
  - `2024-01-31 + P1M → 2024-03-02` (année bissextile).
  De plus, l'échéance **initiale** n'est pas calculée : elle est **saisie par l'utilisateur** ; le cron
  ne fait qu'avancer.
- **Acceptation REQ-SUB-012** : `2025-01-31 → 2025-02-28`, puis `→ 2025-03-31` (ancré au jour d'origine
  + clamp fin de mois).

Ces deux comportements sont incompatibles.

## Décision

**On implémente l'acceptation écrite : ancrage sur le jour d'origine + clamp en fin de mois.** Le
débordement de PHP est un **bug connu** hostile à un suivi de dépenses (échéances qui sautent un mois
et dérivent). L'occurrence *k* est calculée depuis l'**ancre** (date de premier paiement) :
`occurrence(k) = ancre + k × intervalle mois`, avec clamp au dernier jour du mois si le jour d'origine
n'existe pas (via `NaiveDate::checked_add_months`, qui clampe). La prochaine échéance est la plus petite
occurrence strictement postérieure à la date de référence — le calcul depuis l'ancre garantit le retour
au 31 (mars) après un clamp au 28 (février), sans rester ancré au 28.

Le modèle est une **date** (`NaiveDate`, comme Wallos qui stocke une `DATE`) : il est **immunisé aux
changements d'heure** par construction (une date calendaire n'a pas d'heure). Le critère « heure locale
de facturation préservée » est donc trivial au niveau date ; la facturation à l'heure près est hors
périmètre (cf. revue, différé).

## Conséquences

- **Divergence assumée avec l'oracle legacy** : l'e2e de SUB-012 est `@design` (pas de rejeu
  `TARGET=legacy`), conformément à ADR 0016 / OQ-007. La fixture
  `e2e/fixtures/oracles/REQ-SUB-012-monthly.json` fige **les deux** : le comportement Wallos capturé
  (pour mémoire/traçabilité) ET la règle subtrack retenue (réinjectée dans les tests `core`).
- **Portée aux cycles non mensuels (SUB-013)** : la même règle ancrage+clamp s'appliquera par cohérence
  (année : 29 févr → 28 févr en clamp), mais SUB-013 tranchera formellement sa propre divergence oracle.
- **Import depuis Wallos (SUB-016)** : les `next_payment` importés seront recalculés selon la règle
  subtrack, ce qui peut décaler une échéance héritée d'un débordement PHP — à documenter à ce moment-là.
