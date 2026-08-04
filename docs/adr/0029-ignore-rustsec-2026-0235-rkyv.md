# ADR 0029 — Ignorer RUSTSEC-2026-0235 (`rkyv 0.7`, non compilé)

- **Statut** : accepté (2026-08-04)
- **Contexte** : la porte `cargo audit` interroge la base d'avis RustSec **à jour à chaque exécution** ;
  l'avis RUSTSEC-2026-0235 est apparu et a fait échouer la CI d'une PR **sans rapport** (re-cadrage
  d'AUT-005). Même mécanique que ADR 0020.

## Problème

RUSTSEC-2026-0235 : « Insufficient archive validation can cause out-of-bounds reads in archives
containing Rc/Arc » — `rkyv 0.7.46`. Solution annoncée : passer à `rkyv >= 0.8.17`.

`rkyv` est tiré comme **feature optionnelle** de `rust_decimal` (notre type monétaire). Il figure dans
`Cargo.lock` mais **n'est pas compilé** dans notre build : `cargo tree -i rkyv` ne remonte aucun
consommateur actif. Nous n'utilisons **jamais** rkyv, donc jamais la désérialisation d'archives rkyv
— la seule surface concernée par l'avis. La vulnérabilité est **non exploitable** dans wallos-rs.

Le correctif (`rkyv 0.8`) est une **version majeure incompatible** avec la contrainte `rkyv 0.7` de
`rust_decimal 1.37` : impossible à appliquer sans que `rust_decimal` migre en amont.

## Décision

**Ignorer RUSTSEC-2026-0235** dans les deux portes (`deny.toml` et l'étape `cargo audit`), avec cette
justification. À **réévaluer** dès qu'une version de `rust_decimal` supprime rkyv 0.7 (montée du lock)
ou que l'avis reçoit un correctif rétroporté en 0.7 — auquel cas l'ignore sera retiré.

Ne concerne **que** cet avis précis : les autres advisories restent bloquantes (aucun affaiblissement
général de la porte, R3).

## Note de gouvernance

`cargo audit` sur la base live est un **risque récurrent** : n'importe quelle nouvelle advisory peut
rougir la CI, y compris sur une PR documentaire. Piste à considérer si cela se répète : figer la base
d'avis (`--db` épinglé) et la mettre à jour délibérément, plutôt que de suivre le flux en continu. Suivi
dans `spec/OPEN-QUESTIONS.md` (OQ-012 voisin).
