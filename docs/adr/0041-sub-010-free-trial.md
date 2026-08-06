# ADR 0041 — Période d'essai gratuit : concept introduit par conception (absent de Wallos)

- **Statut** : accepté (2026-08-06)
- **Contexte** : REQ-SUB-010 (« période d'essai gratuit »), marquée `oracle: legacy`, criticality medium,
  layer `[core, api, ui]`, e2e required, dépend de REQ-SUB-001 et REQ-NOT-001.

## Problème

Deux critères : (#1) un abonnement en essai n'est **pas compté** dans les statistiques tant que l'essai
n'est pas terminé ; (#2) quand la fin d'essai approche du délai de rappel, une **notification distincte**
du rappel d'échéance est émise. L'exigence est marquée `oracle: legacy` — mais que capturer ?

## Constat : Wallos n'a pas d'essai gratuit

Inspection de l'image épinglée (`bellamy/wallos@sha256:316f…789f`) : la table `subscriptions` n'a
**aucune** colonne d'essai (seulement `start_date`, `next_payment`, `cancellation_date`), et
`grep -ri 'trial|free_trial'` sur `/var/www/html` est **vide**. Wallos 5.4.2 **ne connaît pas** la
période d'essai. Il n'y a donc **pas de comportement de référence** à capturer : comme pour l'hypothèse
corrigée d'ADR 0016/0030, l'exigence est traitée **par conception** (design), à partir des seuls critères
d'acceptation. Constat gelé dans `e2e/fixtures/oracles/REQ-SUB-010-trial.json`.

## Décision

### Modèle : `trial_end_date` nullable

Un abonnement porte une **fin d'essai** optionnelle (`subscriptions.trial_end_date`). Il est **en essai**
tant que `today < trial_end_date` ; l'essai est **terminé** à partir de `today >= trial_end_date`
(`Subscription::is_in_trial`, pur). **Aucune contrainte de position** vis-à-vis du premier paiement : un
essai **précède** typiquement la facturation, mais le modèle ne l'impose pas (contrainte initiale
inversée puis retirée).

### Critère #1 : exclusion des agrégats de coût

Un abonnement en essai contribue **0** à tous les agrégats : `core::billable_amounts` l'exclut (au même
titre qu'un abonnement désactivé ou terminé), et côté serveur le **total** (`active_amounts`), la
**répartition** et l'**évolution** l'excluent — pour l'évolution, la fenêtre de coût démarre à la **fin
d'essai** (le coût n'apparaît qu'une fois l'essai terminé). Le DTO expose `trial_end` et un drapeau
dérivé `in_trial` (horloge serveur).

### Critère #2 : rappel de fin d'essai distinct

Le cron de rappel (REQ-NOT-001) émet, en plus du rappel de paiement, un rappel de **fin d'essai** quand
`trial_end_date` est à exactement le délai de rappel du foyer. Les deux types sont distingués par
`reminder_log.kind` (`payment` / `trial_ending`) — unicité anti-doublon incluant le type, et champ `kind`
dans la vue `GET /reminders`.

## Conséquences

- `Subscription::with_trial_end`/`trial_end`/`is_in_trial` (core), migration `0022`
  (`trial_end_date` + `reminder_log.kind`), persistance create/update/list/get, exclusion dans tous les
  agrégats, `trial_end`/`in_trial` dans `SubscriptionDto`, `trial_end` dans `CreateSubscriptionRequest`
  (round-trip export/import préservé). UI : champ de formulaire + badge « essai ».
- Le rappel de fin d'essai réutilise l'infrastructure NOT-001 (aucun endpoint nouveau).
- `oracle: legacy` de l'exigence est **factuellement inexact** pour cette fonctionnalité (Wallos ne l'a
  pas) : traité comme `design`, à l'image d'ADR 0016.
