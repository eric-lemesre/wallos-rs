# ADR 0054 — Nettoyage du périmètre natif (exécution des conséquences d'OQ-009)

- **Statut** : accepté (2026-08-08) — exécution demandée par Eric (« traiter les OQ ouvertes »)
- **Contexte** : OQ-009 (décision du 2026-08-04) : la cible est la **parité** avec Wallos, qui n'a
  ni desktop ni mobile natifs — le natif est **hors périmètre**, non différé. La décision était
  actée ; ses conséquences 2 et 3 restaient « à ordonnancer » (la 1, REQ-AUT-005, avait été soldée
  par OQ-011/ADR 0028).

## Exécution

### Conséquence 2 — REQ-SEC-006 recadrée

Le critère « configuration Tauri : seules les capacités utilisées sont accordées » est **retiré**
de la spec (il portait sur un artefact qui n'existera pas). Le volet **CSP web** — implémenté et
testé — demeure seul ; le statut `verified` reste exact. Aucune modification de code : l'exigence
n'a jamais eu d'implémentation Tauri.

### Conséquence 3 — périmètre mort supprimé

- **`crates/desktop`** (squelette Tauri v2, 9 lignes) et **`crates/client`** (squelette SDK HTTP,
  26 lignes, dont le seul consommateur prévu était le desktop) : **supprimés** — retirés des
  membres du workspace, des dépendances et des exclusions de traçabilité
  (`xtask/trace-exclusions.toml`). Un SDK Rust pourra renaître avec un vrai consommateur.
- **`frontend/platform`** et **`frontend/shells/{desktop,mobile}`** : n'avaient jamais été créés —
  seules leurs mentions documentaires existaient.
- **AGENTS.md** nettoyé chirurgicalement (numérotations conservées) : règle **R7** marquée retirée,
  arborescence, principe frontend (une coquille web), bloc `PlatformAdapter` supprimé, stratégie
  e2e réduite à **L1** (L2 `tauri-driver` et L3 Maestro retirés), porte CI 12 marquée retirée,
  tableau des sessions (ligne « Desktop & mobile » → « API (intégrations) », renvoi ADR 0028),
  hypothèse H3 close, justification du routage `hash` reformulée honnêtement (choix hérité,
  conservé à coût nul).
- **`wallos_proto::DeviceToken`** : la doc ne référence plus `PlatformAdapter.secureStore`
  (« le client doit le conserver de façon sûre, jamais en stockage web lisible »).

## Ce qui ne change pas

- La **PWA responsive** reste la modalité mobile (OQ-006) ; le routage `hash` est conservé.
- Les jetons d'appareil (AUT-005 re-cadré) restent une capacité API vivante et testée.
- Une divergence fonctionnelle future (vouloir du natif au-delà de la parité) rouvrirait le sujet
  par une nouvelle OQ — ce nettoyage n'hypothèque rien : les 35 lignes supprimées étaient vides
  de comportement.
