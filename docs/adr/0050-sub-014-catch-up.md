# ADR 0050 — Rattrapage des échéances : convergence par calcul, jamais par rejeu

- **Statut** : accepté (2026-08-07)
- **Contexte** : REQ-SUB-014 (« Rattrapage des échéances passées »), `oracle: legacy`, criticality
  **high**, layer `[core, api]`, e2e required. Un client hors ligne plusieurs semaines doit
  converger sans produire de rappels rétroactifs en rafale.

## Décision : les deux garanties sont déjà structurelles

Comme NOT-002 (ADR 0048), cette exigence est portée par des mécanismes existants ; ce cycle ajoute
les annotations, les preuves et le scénario e2e — aucun code de production nouveau.

### (1) La prochaine échéance est calculée, pas stockée-avancée

Le legacy stocke `next_payment` et l'avance par un cron (`updatenextpayment.php` : « add intervals
until future »). subtrack **calcule** l'échéance à la demande : `core::next_due(anchor, cycle,
after)` itère depuis l'**ancrage** (ancrage+clamp, ADR 0022) et renvoie la première occurrence
**strictement postérieure** à `after` — le rattrapage de N occurrences dépassées est le
comportement nominal de la fonction, pas un travail de rattrapage à planifier. Même convergence
que l'oracle, plus une borne anti-pathologique (`MAX_STEPS = 100 000`) que le legacy n'a pas
(sa boucle `while` est non bornée). Aucune donnée à réparer : rien n'est jamais « en retard ».

### (2) Aucune rafale rétroactive

L'ordonnanceur (NOT-001/002) évalue les rappels **du jour** uniquement : `due_reminders` exige
`days_until == lead` exactement, et `candidate_of` repart de `next_due` — donc d'une occurrence
future. Les N occurrences manquées n'existent tout simplement pas aux yeux du cron. Preuve
d'intégration : abonnement ancré 5 mois dans le passé, balayage → zéro émission, zéro requête
sortante.

## Conséquences

- Annotations `REQ-SUB-014` sur `core::next_due` et `computeNextDue` (+ `x-requirements`).
- Tests core (18 occurrences mensuelles sautées ; hebdomadaire sans dérive), intégration (cron
  sans rafale ; API `next-due` strictement future), e2e `@REQ-SUB-014` (calculateur UI, ancrage
  dynamique à −150 j).
- La dépendance circulaire de spec NOT-002 ↔ SUB-014 se résout d'elle-même : les deux exigences
  décrivent les deux faces du même invariant (« le passé n'est jamais rejoué »), l'une côté
  ordonnanceur, l'autre côté calcul d'échéance.
