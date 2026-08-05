# ADR 0031 — Répartition par catégorie et par payeur : agrégation à somme conservée + bucket « (aucun) » explicite

- **Statut** : accepté (2026-08-05)
- **Contexte** : REQ-STA-004 (« répartition par catégorie et par payeur »), exigence `oracle: legacy`,
  criticality medium, layer `[core, api, ui]`, dépend de REQ-STA-001, REQ-CAT-001, REQ-SUB-017 (tous
  verified). Jumeau décidé avec SUB-017 (OQ-010 : dépendance inversée STA-004 → SUB-017).

## Problème

L'exigence demande la répartition des coûts mensuels sur **deux axes** (catégorie, payeur), avec deux
critères d'acceptation : (#1) la **somme des parts d'un axe égale le total général sans écart
d'arrondi** ; (#2) les abonnements **sans catégorie / sans payeur** sont regroupés dans une **entrée
explicite, jamais omise**. Deux points à trancher : la **mécanique d'agrégation** (fidèle au legacy) et
la **représentation de l'absence d'axe** (le modèle subtrack diverge du legacy).

## Décision

### Mécanique : accumulation en précision pleine (oracle legacy)

Comportement capturé sur Wallos 5.4.2 (`includes/stats_calculations.php`, boucle d'agrégation) et
**gelé** dans `e2e/fixtures/oracles/REQ-STA-004-repartition.json` : chaque abonnement **actif**
(`inactive == 0`) ajoute son coût mensuel normalisé (`getPricePerMonth`, REQ-STA-001) **converti** dans
la devise principale (`getPriceConverted`, REQ-CUR-003) à `categoryCost[category_id]`,
`memberCost[payer_id]` **et** `totalCostPerMonth`. Chaque abonnement compte dans **exactement un**
bucket par axe, d'où l'invariant `somme(categoryCost) == totalCostPerMonth == somme(memberCost)`.

subtrack calque cette mécanique dans le **domaine pur** (`core::repartition`) : agrégation par clé, en
**précision décimale exacte** (R4), l'arrondi d'affichage n'intervenant qu'**une seule fois** sur chaque
part et sur le total (REQ-CUR-005/007) — jamais sur l'accumulation. L'invariant #1 est prouvé par un test
`proptest` (somme des parts == somme des contributions) et asséré en intégration sur l'exemple gelé.
Comme sur le legacy, seuls les abonnements **actifs** pèsent ; subtrack exclut en plus les abonnements
**terminés** à la date du jour (REQ-SUB-009), cohérent avec tous ses autres agrégats
(`core::billable_amounts`) — divergence sans objet côté legacy (Wallos n'a pas de date de fin dans ce
calcul).

### Absence d'axe : bucket « (aucun) » explicite (design)

**Divergence de modèle assumée.** Wallos n'a **pas** de NULL : chaque abonnement porte toujours un
`category_id` (défaut = catégorie sentinelle « No category ») et un `payer_user_id` (défaut = membre
id=1). subtrack rend `category_id`/`payer_id` **nullables** (aucune catégorie ni payeur par défaut
auto-affecté hors seed REQ-CAT-002). Une valeur `None` est agrégée dans un **unique bucket explicite**
(`core::RepartitionShare { key: None }`), exposé côté API par `label = null` et rendu par l'interface via
un libellé **localisé** « (aucun) » (`repartition.none`) — **jamais omis** (critère #2). Cela préserve
l'invariant #1 : un abonnement sans axe alimente le total *et* une part visible.

Petite divergence d'affichage documentée : Wallos **omet** du graphe toute entrée de coût exactement nul
(`if ($cost != 0)`) ; subtrack ne construit un bucket qu'à partir d'abonnements réellement contributeurs
(jamais de bucket vide) et conserve un bucket de coût nul s'il contient au moins un abonnement (prix 0),
pour honorer « jamais omis ». L'invariant somme=total est insensible aux entrées nulles.

### Surface API : une réponse, deux axes

Un seul endpoint `getRepartition` (`GET /statistics/repartition`) renvoie les deux axes
(`by_category`, `by_payer`) plus le `total` général et un drapeau `complete` — un aller-retour pour la
page de statistiques, calqué sur `getCostEvolution`. Un abonnement **non convertible** (devise illisible
ou taux manquant, REQ-CUR-003) est **exclu des deux axes** et bascule `complete = false` — jamais
assimilé à un coût nul silencieux (cohérent avec `CostEvolutionResponse.complete`). Tri des parts par
**coût décroissant** (comme le legacy, `usort $b['y'] <=> $a['y']`), avec départage déterministe stable
(nombre de contributeurs, puis clé) pour un rendu reproductible sur totaux égaux.

## Traçabilité de l'oracle

Conforme au protocole §8.1 tel que pratiqué : oracle **capturé manuellement** depuis l'image épinglée
(`bellamy/wallos@sha256:316f…789f`) et **gelé** en fixture JSON avec `_source` (fonctions PHP) ; la
mécanique et l'invariant sont assérés au **niveau intégration** (`repartition_sums_to_the_grand_total`)
et un scénario e2e `@design` vérifie l'UI subtrack (composition par foyer et bucket « (aucun) » non
rejouables sur Wallos mono-foyer — même traitement que REQ-CAT-003 / REQ-SUB-017). Wallos n'est **pas**
exécuté en CI.

## Conséquences

- Nouvelle opération `getRepartition` (couverture API + authz 100 % : trio owner/other/anon).
- Le formulaire d'abonnement supporte déjà le rattachement à une catégorie et à un payeur (sélecteurs) ;
  la carte de répartition consomme ces rattachements. Un sélecteur de payeur au formulaire pourra être
  ajouté ultérieurement si besoin (l'API le supporte).
- Clôt la paire catégorie/payeur (SUB-017 + STA-004) : la **parité** des statistiques de répartition est
  atteinte.
