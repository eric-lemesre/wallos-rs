# ADR 0021 — Exemptions justifiées de TRC-06 (`xtask/trace-exclusions.toml`)

## Contexte

La règle **TRC-06** (`cargo xtask trace`, AGENTS.md §5) signale « un fichier de production sans aucune
annotation `#[requirement(...)]` / `#[verifies(...)]` » — l'objectif est d'interdire le **code
orphelin** (logique métier non rattachée à une exigence).

Or l'implémentation flaguait **tout** fichier `.rs` sous `crates/`, y compris des fichiers qui ne
contiennent **aucune logique annotable** :
- **racines de module** (`crates/core/src/lib.rs`, `crates/storage/src/lib.rs`) : uniquement
  `pub mod` / `pub use` ;
- **types d'erreur** (`crates/core/src/error.rs`, `crates/storage/src/error.rs`) : un `enum` +
  `#[derive(Error)]` ;
- **crates stub** (`notifier`, `client`, `desktop`) : squelettes sans exigence encore implémentée.

Ces fichiers **ne peuvent pas** être annotés : la macro `#[requirement]` ne s'applique qu'aux `fn`
(`parse_macro_input as ItemFn`) — un module ou un `enum` n'a pas de fonction. La porte `trace` était
donc **rouge en permanence** (le job CI `ci` s'arrêtait là), indépendamment de l'avancement des
exigences.

## Décision

Introduire **`xtask/trace-exclusions.toml`** — une liste explicite de fichiers exemptés de TRC-06,
sur le modèle de `xtask/coverage-exclusions.toml` (§6) : **chaque entrée porte une justification**
(commentaire). `cargo xtask trace` charge ce fichier (parseur minimal, sans nouvelle dépendance) et
saute ces chemins dans la boucle TRC-06.

**Discipline** : une entrée est **retirée** dès qu'une exigence est réellement implémentée dans le
fichier (il portera alors une annotation). Les crates stub y figurent temporairement, à retirer quand
leur domaine est livré.

## Conséquences

- La porte `trace` peut redevenir **verte** : elle ne flague plus que le vrai code orphelin (fichiers
  hors liste, avec logique, sans annotation).
- La règle reste **stricte et auditable** : la liste est courte, versionnée et justifiée ; aucun
  contournement silencieux (pas de `#[allow]` en ligne, pas de skip heuristique opaque).
- Portée limitée à TRC-06 ; les autres codes TRC (01/02/03/04/05/07) sont inchangés.
- Modification de l'outillage `xtask` — cohérente avec ADR 0001 (intégrité xtask) : on **précise** la
  règle sans en abaisser l'intention.

## Liens

- AGENTS.md §0 (R1, protocole), §5 (codes TRC) ; ADR 0001 (intégrité xtask), §6 (exclusions de
  couverture justifiées, pattern repris ici). `xtask/trace-exclusions.toml`, `xtask/src/main.rs`
  (module `trace`).

## Statut

accepted
