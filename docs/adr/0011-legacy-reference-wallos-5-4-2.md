# ADR 0011 — Version de référence de l'application d'origine gelée à Wallos 5.4.2

## Contexte

`OQ-001` restait ouverte et bloquait **toutes** les exigences `oracle: legacy` : le protocole
d'oracle (AGENTS.md §8.1) exige une cible de référence **immobile**. Toute montée de version de
l'application d'origine en cours de projet invaliderait silencieusement les fixtures d'oracle
regelées, sans qu'aucune porte ne le détecte.

L'application d'origine est **Wallos** (`github.com/ellite/Wallos`), suivi de dépenses récurrentes
auto-hébergé dont le modèle métier (abonnements, catégories, devises, payeurs, moyens de paiement,
canaux de notification email/webhook/telegram/discord/gotify/pushover) constitue le référentiel
`oracle: legacy` de ce projet.

Les options recensées étaient : A) figer un tag Docker précis pour toute la durée du projet —
B) suivre la dernière version et rejouer l'enregistrement des oracles à chaque montée. La
recommandation agent était A.

## Décision

Le responsable du dépôt a arbitré : **figer la dernière version stable disponible à la date du
2026-07-25, jusqu'à l'implémentation complète du projet** (option A).

La cible de référence gelée est :

- **Image** : `bellamy/wallos`
- **Tag** : `5.4.2` (= release GitHub `v5.4.2`, publiée le 2026-07-19, dernière stable au 2026-07-25)
- **Digest (référence faisant foi)** :
  `sha256:316f26e13265958e7946ef98ff600516fddc51d698ee98bd1ae1577e5e00789f`

Le **digest** est la référence contraignante : un tag peut être re-poussé, un digest est immuable.
Toute cible `LegacyDriver` (AGENTS.md §8.1) et tout `docker-compose` d'oracle **doivent** épingler
l'image par digest :

```
image: bellamy/wallos:5.4.2@sha256:316f26e13265958e7946ef98ff600516fddc51d698ee98bd1ae1577e5e00789f
```

Cette version reste gelée pour toute la durée du projet. Toute montée exige un **nouvel ADR** et le
rejeu complet des oracles (`pnpm e2e:record`), conformément à §8.1.

## Conséquences

- Débloque toutes les exigences `oracle: legacy` : domaine SUB (modèle, cycles, calcul d'échéance),
  STA-001..005, CAT, CUR-001/005/007, NOT-001/003/004/005/006, I18N-001.
- L'arborescence `e2e/` (absente à ce jour) devra matérialiser ce pin dans la configuration du
  `LegacyDriver` et le `docker-compose` de la cible legacy — une seule source pour le digest.
- L'hypothèse H7 d'`AGENTS.md` passe de ⬜ (défaut) à ✅ (arbitrée).
- Rappel §8.1 : un scénario `oracle: legacy` doit d'abord **passer** contre `TARGET=legacy` avant
  d'être gelé ; s'il échoue, c'est la compréhension du comportement de référence qui est fausse,
  jamais l'application d'origine.

## Liens

- AGENTS.md §0, §8.1, §8.3, H7 ; `spec/OPEN-QUESTIONS.md` : OQ-001 (résolue par cet ADR).
- Source : `github.com/ellite/Wallos` release `v5.4.2` ; image `hub.docker.com/r/bellamy/wallos`.
- Exigences débloquées : tout `oracle: legacy` (voir `spec/requirements.lock.yaml`).

## Statut

accepted
