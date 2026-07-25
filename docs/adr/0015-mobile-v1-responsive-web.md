# ADR 0015 — Modalité mobile v1 : web responsive installable, coquille native reportée

## Contexte

`OQ-006` restait ouverte et conditionnait le niveau **L3** de la stratégie E2E (AGENTS.md §8.2).
La coquille mobile native Tauri implique signature, magasins d'applications et permissions natives
— un coût non couvert par la génération de code, et de faible valeur tant que l'UI partagée
(`frontend/ui`) n'est pas stabilisée.

Options recensées : A) web responsive installable en v1, coquille native reportée — B) coquille
native dès la v1. Recommandation agent : A.

## Décision

Le responsable du dépôt a arbitré **l'option A**.

- La v1 livre une **application web responsive installable** (PWA) couvrant la modalité mobile.
- La **coquille native mobile** (Tauri iOS/Android, `frontend/shells/mobile`) est **reportée**
  après stabilisation de l'UI partagée.

## Conséquences

- Le niveau **L3** de la stratégie E2E reste conforme à §8.2 : émulation de viewport Playwright
  (rendu responsive) ; le smoke natif Maestro est **hors périmètre v1**.
- Aucun point d'entrée `frontend/shells/mobile` n'est requis en v1 ; le budget de code par coquille
  (§7, ≤ 300 lignes) ne s'applique qu'aux coquilles web et desktop pour l'instant.
- Les niveaux **L1** (web, chromium + webkit) et **L2** (desktop Tauri) restent pleinement dans le
  périmètre v1.
- Réversible par ADR : l'introduction ultérieure de la coquille native mobile ne remet pas en cause
  l'UI partagée, seulement l'ajout d'une coquille mince.

## Liens

- AGENTS.md §0, §7, §8.2 (niveaux L1/L2/L3) ; `spec/OPEN-QUESTIONS.md` : OQ-006 (résolue par cet ADR).
- Lié : OQ-002 (foyer) et OQ-005 n'affectent pas ce choix.

## Statut

accepted
