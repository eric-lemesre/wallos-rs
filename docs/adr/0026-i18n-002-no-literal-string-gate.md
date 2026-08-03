# ADR 0026 — Absence de chaîne littérale : porte xtask + clés i18next typées

- **Statut** : accepté (2026-08-03)
- **Contexte** : REQ-I18N-002 (« absence de chaîne littérale dans le code »), exigence `oracle: design`,
  criticality medium, layer `[ui]`, dépend de REQ-I18N-001 (choix et persistance de la langue).

## Problème

L'acceptance impose deux garanties, à propos du seul `frontend/ui` :

1. **Aucune chaîne destinée à l'affichage hors des catalogues de traduction.**
2. **Une clé de traduction absente du catalogue de référence fait échouer la construction.**

Le rationale de l'exigence est explicite : « un agent produit spontanément des libellés en dur ; sans
porte automatique, la traduction se dégrade à chaque itération ». Il faut donc un contrôle **exécuté en
CI**, pas une simple convention.

Au moment de l'implémentation, `frontend/ui` **ne possédait aucun ESLint** (ni configuration, ni script,
ni dépendance) et le code était déjà conforme (aucune violation existante).

## Décision

Deux mécanismes complémentaires, un par critère, sans nouvelle dépendance externe :

### Critère 2 — clés typées captées par `tsc`

Augmentation de `CustomTypeOptions` d'i18next (`frontend/ui/src/i18n/i18next.d.ts`) avec le **catalogue
anglais comme référence** : `t("…")` n'accepte plus qu'une clé existante, et `tsc --noEmit` (porte
`typecheck` déjà en CI, job `frontend`) échoue sur toute clé inconnue. Les littéraux de clés écrits
**hors** de `t(...)` — les messages de validation des schémas zod — sont typés via un helper identité
`tKey(key: ParseKeys)` (`frontend/ui/src/i18n/keys.ts`), de sorte qu'aucun littéral n'échappe au
contrôle. Aux points de rendu où react-hook-form élargit le message à `string`, un cast `as ParseKeys`
réaffirme ce que `tKey` a déjà garanti à la source.

### Critère 1 — porte `cargo xtask lint-i18n`

Plutôt qu'introduire tout l'outillage ESLint (plusieurs `devDependencies`, nouvelle configuration, nouveau
job), on ajoute une porte **cohérente avec la famille existante** `lint-money` / `lint-clock` : une
analyse ligne à ligne des composants `.tsx` (hors `*.test.tsx`) signalant deux motifs à **haute valeur de
signal** :

- un **nœud texte JSX** : du texte alphabétique entre `>` et une balise fermante `</` ;
- un **attribut d'affichage** littéral (`title`/`placeholder`/`alt`/`aria-label`) valué par une chaîne
  entre guillemets contenant des lettres.

L'heuristique est **volontairement conservatrice** (pas un parseur JSX complet) et calibrée pour **zéro
faux positif** sur le code conforme : la contrainte « balise fermante `</` » exclut les génériques
TypeScript (`Promise<void>`), et le début `{` exclut les expressions `{t("…")}`. Elle attrape les
régressions les plus courantes (`<h2>Bonjour</h2>`, `placeholder="Rechercher"`). La fonction d'analyse
`scan` est pure et couverte par cinq tests unitaires (littéral détecté, générique/expression/attributs
non-affichage ignorés).

## Alternatives écartées

- **ESLint + `eslint-plugin-i18next`** : c'est l'outil standard, mais il aurait introduit plusieurs
  dépendances (ADR requis, R6), une configuration et un job entiers pour une exigence `medium`, alors
  qu'aucun ESLint n'existe encore. Réservé à une future mise en place transverse (qui servira aussi R7,
  la restriction d'import `@tauri-apps`).

## Traçabilité

Contrairement au code de production Rust, cette exigence est **`layer: [ui]`** et n'a **pas de code Rust
de production** : la traçabilité suit la convention frontend du dépôt, le **JSDoc `@implements
REQ-I18N-002`** (dans `i18next.d.ts`, `keys.ts`, `index.ts`). Le proc-macro `#[requirement]` n'est pas
applicable ici : il calcule la racine du workspace en supposant une profondeur `crates/<nom>`, incompatible
avec `xtask/` (profondeur 1). La porte `trace` n'exige d'ailleurs les annotations `#[requirement]` /
`#[verifies]` que pour le statut `accepted` ; l'exigence est promue directement `verified`, son
implémentation **et** sa vérification étant les deux portes CI (`typecheck` + `lint-i18n`) et leurs tests.

## Conséquences

- Nouvelle porte CI `cargo xtask lint-i18n` (job `ci`) ; `typecheck` (job `frontend`) couvre désormais
  aussi la validité des clés.
- Débloque REQ-I18N-004 (dépendait de REQ-I18N-002).
- **Limite assumée** : l'heuristique ne couvre pas les nœuds texte multi-lignes ni la concaténation
  dynamique ; c'est un garde-fou anti-régression, pas une preuve d'exhaustivité. Une bascule ultérieure
  vers ESLint pourrait la remplacer sans changer le contrat de l'exigence.
