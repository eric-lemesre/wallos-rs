# ADR 0024 — Recherche et tri des abonnements : plein-texte replié + tri sur montant normalisé (extension de l'oracle legacy)

- **Statut** : accepté (2026-07-31)
- **Contexte** : REQ-SUB-007 (recherche et tri), exigence `oracle: legacy`, criticality medium,
  layer `[api, ui]`, dépend de SUB-006 ✓ + STA-001 ✓.

## Problème

L'acceptance SUB-007 demande deux capacités :

1. une **recherche** dont la correspondance est « insensible à la casse et aux diacritiques sur le nom
   **et** les notes » ;
2. un **tri** par nom, montant ou prochaine échéance, où « le tri par montant s'effectue **après
   normalisation en devise de référence** » et où « l'ordre est stable ».

La **capture** de l'oracle (Wallos 5.4.2, `endpoints/subscriptions/get.php`) révèle trois frictions
avec la spec et le modèle de subtrack :

1. Wallos **n'a aucune recherche plein-texte** sur les abonnements : ses filtres sont catégorie / moyen
   de paiement / membre / état / type de renouvellement / notifications. La « recherche » de son UI
   (`scripts/subscriptions.js`) ne cible que les **logos**.
2. Le tri Wallos est un `ORDER BY` **SQL** sur colonnes brutes ; le tri par prix porte sur le **montant
   stocké** (aucune normalisation de cycle, aucune conversion de devise) en **descendant**. Comparer un
   mensuel à un annuel, ou deux devises différentes, y est dénué de sens.
3. Le tri Wallos par défaut est `next_payment ASC` ; le tri par nom s'appuie sur la **collation SQL**
   (sensibilité aux diacritiques variable selon la collation).

## Décision

**On étend délibérément le comportement legacy, en documentant les divergences dans la fixture oracle
`e2e/fixtures/oracles/REQ-SUB-007-search-sort.json`.**

1. **Recherche plein-texte repliée, côté serveur.** Un paramètre `?search=<terme>` filtre les
   abonnements dont le **nom OU les notes** contiennent le terme, en comparaison **insensible casse +
   diacritiques**. Le repli vit dans `core` (`text::fold_for_search` : décomposition NFD, suppression
   des marques combinantes U+0300..=U+036F, minuscule ; périmètre écriture latine en/fr — limitation
   assumée pour ø/ß qui n'ont pas de forme décomposée). Le filtrage s'applique **avant** l'agrégat et le
   tri : le total reflète exactement la vue recherchée (cohérent avec REQ-STA-007). Requête vide/absente
   ⇒ aucun filtrage.

2. **Tri par montant = coût mensuel normalisé converti en devise de référence.** `?sort=amount` trie sur
   le **coût mensuel** (REQ-STA-001) de chaque abonnement **converti dans la devise de référence du
   foyer** (REQ-CUR-001/003), en **décroissant** (les plus coûteux d'abord, esprit du `price DESC`
   legacy). Un abonnement au coût non calculable (devise/cycle illisible, taux manquant) est placé **en
   fin de liste**, jamais assimilé à un coût nul (revue STA-001 F2). C'est précisément ce qui justifie la
   dépendance à STA-001.

3. **Défaut = nom replié, croissant ; tri tolérant.** Le tri par défaut est le **nom** (replié,
   croissant), aligné sur l'ordre de liste SUB-006 déjà figé — pas `next_payment` comme Wallos. Le tri
   étant un confort d'affichage, une valeur `?sort=` inconnue **retombe silencieusement sur le nom**
   (jamais 422). `?sort=next_due` trie par prochaine échéance dérivée (ancrage+clamp, ADR 0022), les
   abonnements sans échéance à venir en fin de liste.

4. **Départage stable et déterministe.** Pour tous les critères, départage par **nom replié puis
   identifiant** (esprit REQ-CAT-005), garantissant un ordre reproductible.

## Conséquences

- **Pas de nouvelle opération OpenAPI** : `search` et `sort` sont deux paramètres de query ajoutés à
  `listSubscriptions` (opération SUB-006 existante). La couverture authz reste 100 % (les trois tests
  owner/other/anon de `listSubscriptions` couvrent l'opération étendue) ; aucun nouveau test authz requis.
- **Tri appliqué en Rust, plus en SQL** : le tri se fait désormais après projection (le montant
  normalisé et l'échéance sont calculés côté serveur), l'`ORDER BY name, id` du repository ne sert plus
  que de base stable. Le jeu de données d'un foyer est borné (suivi personnel), le coût est négligeable.
- **Taux chargés à la demande** : la table de conversion n'est chargée que si un montant actif doit être
  agrégé **ou** si `sort=amount` est demandé (sinon aucun aller-retour base, revue SUB-006 #8).
- **Dépendance ajoutée** : `unicode-normalization` (déjà présente transitivement) devient une dépendance
  directe de `core`, pour la décomposition NFD du repli de recherche.
- **e2e** : `e2e: required` ; vérifié en `@design` (la recherche texte et le tri normalisé divergent
  délibérément du legacy, aucun rejeu `TARGET=legacy`). Couverture core (unitaire `text`) + api
  (intégration `subscriptions`) + e2e (`subscription-search-sort.spec.ts`).
