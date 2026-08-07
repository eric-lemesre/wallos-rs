# ADR 0051 — Formatage localisé : Intl natif, locale = langue de l'interface, dates civiles

- **Statut** : accepté (2026-08-07)
- **Contexte** : paire circulaire REQ-CUR-006 (« Formatage localisé des montants ») ↔ REQ-I18N-003
  (« Formats de date et de nombre localisés »), layer `[ui]`, e2e required, oracle design —
  traitées ensemble (comme SUB-017/STA-004). Avant ce cycle, l'UI concaténait la chaîne décimale
  brute et le code ISO (`9.99 EUR`) et affichait les dates `YYYY-MM-DD` telles quelles.

## Décisions

### `Intl` natif, pas de bibliothèque

`Intl.NumberFormat` / `Intl.DateTimeFormat` portent les données CLDR (séparateurs, position du
symbole, décimales ISO 4217 par devise, noms de mois) dans tous les navigateurs cibles et dans
Node (tests). Aucune dépendance ajoutée. Une devise sans sous-unité (JPY) n'affiche aucune
décimale **par construction** (critère CUR-006 #2) — le champ `decimals` du serveur reste la
référence documentaire, Intl applique la même table. Devise inconnue d'Intl ou montant illisible →
repli sur la forme brute `montant CODE` (jamais de crash d'affichage, jamais `€0.00` pour un
champ vide).

### Locale = langue de l'interface

L'application n'expose que deux langues (`en`, `fr` — REQ-I18N-001, réglage du compte). La locale
de formatage est `i18n.language` : `fr` applique les conventions françaises, `en` les anglaises.
Pas de réglage de locale séparé (« français avec formats américains ») : cas non demandé,
complexité différée. Les helpers (`lib/format.ts` : `formatAmount`, `formatDate`, `formatMonth`)
sont purs et prennent la locale en paramètre — un futur réglage dédié ne changerait que le point
d'appel.

### Dates civiles reconstruites en local (le piège du fuseau)

`new Date("YYYY-MM-DD")` interprète la chaîne en **UTC minuit** : tout fuseau à l'ouest de
Greenwich afficherait **la veille** (critère I18N-003 #2). Les helpers décomposent la chaîne et
reconstruisent la date en **local** (`new Date(y, m-1, d)`) : le jour affiché est toujours le jour
civil du serveur, quel que soit le fuseau du navigateur. Style `medium` (`Aug 7, 2026` /
`7 août 2026`).

### La valeur ne change jamais

Le transport reste canonique (montants décimaux en chaînes R4, dates `YYYY-MM-DD`) ; seule la
**présentation** est localisée. Les saisies (`<input>`) restent en format canonique — la
localisation des champs de saisie est hors périmètre de cette paire.

## Conséquences

- `frontend/ui/src/lib/format.ts` (+ tests unitaires fr/en, JPY, padding, replis, fuseau).
- Montants branchés dans `SubscriptionsList` (prix, coûts mensuel/annuel, total),
  `UpcomingPaymentsCard`, `ConvertedTotalCard`, `RepartitionCard` (dont `aria-label`),
  `CostEvolutionCard` (dont `aria-label`).
- Dates branchées dans `SubscriptionsList` (prochaine échéance), `UpcomingPaymentsCard`,
  `RemindersCard`, `NextDueCard`, `ConvertedTotalCard` (fraîcheur des taux),
  `NotificationChannelsCard` (livraisons), `CostEvolutionCard` (mois).
- Tests vitest et specs e2e adaptés (assertions sur formats anglais, locale des tests) ; nouveaux
  specs `@REQ-CUR-006` et `@REQ-I18N-003` (bascule en/fr sur la même donnée).
