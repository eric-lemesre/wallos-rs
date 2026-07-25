# ADR 0010 — PostgreSQL côté serveur, SQLite côté client

## Contexte

`OQ-003` (arbitrage de l'hypothèse H3, ⬜) restait ouverte : elle bloquait `REQ-SYN-003`,
le choix des migrations et, par ricochet, toute la persistance (`storage`, `server`) ainsi que
la chaîne `auth → sessions → isolation` (§9 impose 3 tests d'autorisation par `operation_id`,
impossibles sans stockage des comptes).

Les options recensées dans `spec/OPEN-QUESTIONS.md` étaient : A) PostgreSQL uniquement —
B) SQLite uniquement — C) les deux via une abstraction de repository. La recommandation agent
mettait en garde contre un C « runtime-swappable » sur un même déploiement (double surface de
test pour la porte de couverture à 100 %).

Le responsable du dépôt a arbitré : **PostgreSQL pour le serveur, SQLite pour les modalités
desktop et mobile**. Cet arbitrage confirme l'hypothèse H3 d'`AGENTS.md`.

## Décision

- Le **serveur** (`crates/storage`, `crates/server`) persiste sur **PostgreSQL** via `sqlx`.
  Motif : pagination stable par curseur (`REQ-SYN-003`), concurrence de l'ordonnanceur de
  notifications (H6), isolation des comptes multi-utilisateur (`REQ-SEC-001`).
- Les **clients** desktop et mobile persistent sur **SQLite** embarqué (côté `crates/client` /
  `crates/desktop`). Motif : fonctionnement hors ligne (`REQ-SYN-007`), auto-hébergement et
  absence de serveur local, promesse d'origine du produit.
- Ce n'est **pas** une abstraction runtime-swappable (option C écartée) : chaque modalité a un
  moteur figé. La séparation est structurelle, pas configurable au déploiement.
- Les **traits de repository** restent définis dans `core` (AGENTS.md §1), sans aucune
  dépendance au moteur. `crates/storage` en fournit l'implémentation PostgreSQL ; l'éventuelle
  implémentation SQLite cliente vivra côté client, jamais dans `core`.
- Chaque moteur porte sa propre suite de migrations ; elles ne sont pas partagées.

## Conséquences

- La porte de couverture 100 % (§6) s'applique par moteur là où le code existe : le serveur ne
  teste que PostgreSQL, le client ne teste que SQLite. Pas de double exécution du même code.
- `sqlx` (PostgreSQL) et le pilote SQLite client sont des dépendances nouvelles : leur
  introduction effective se fera dans les commits d'implémentation, chacun couvert par l'ADR
  présent (R6) et rattaché à une exigence.
- Débloque `REQ-SYN-001/003`, la chaîne `REQ-AUT-*` et `REQ-SEC-001`. `OQ-004` (rétention des
  pierres tombales) et le curseur `since` (H5) restent à arbitrer séparément.
- L'hypothèse H3 passe de ⬜ (défaut) à ✅ (arbitrée) ; toute réouverture exigera un nouvel ADR.

## Liens

- AGENTS.md §0 (R6), §1 (dépendances, repositories dans `core`), §6, §9, H3, H6.
- `spec/OPEN-QUESTIONS.md` : OQ-003 (résolue par cet ADR).
- Exigences débloquées : REQ-SYN-001, REQ-SYN-003, REQ-AUT-001..009, REQ-SEC-001.

## Statut

accepted
