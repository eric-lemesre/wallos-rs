# ADR 0033 — Repli sur la langue de référence : `fallbackLng` runtime + porte de parité en construction

- **Statut** : accepté (2026-08-05)
- **Contexte** : REQ-I18N-004 (« repli sur la langue de référence »), `oracle: design`, criticality
  low, layer `[ui]`, e2e optional, dépend de REQ-I18N-002 (verified).

## Problème

Acceptation : une clé **absente du catalogue de la langue active** doit afficher la valeur de la
**langue de référence** (jamais une clé brute — interface non trouée), et **l'absence est signalée en
construction**. Deux volets : un comportement d'**exécution** (repli) et un signalement **statique**.

## Décision

### Repli d'exécution : `fallbackLng` (déjà en place)

i18next est configuré avec `fallbackLng: "en"` (`frontend/ui/src/i18n/index.ts`). Une clé manquante
dans la langue active est résolue sur la valeur **anglaise** — `en` est la **langue de référence** (et
la langue par défaut, `lng: "en"`). Aucun changement de comportement n'était nécessaire ; la décision
est de **désigner `en` comme référence** et de le documenter (`@implements REQ-I18N-004`). Un test
frontend (`i18n/fallback.test.ts`) vérifie le repli sur une instance i18next à catalogue actif
incomplet.

### Signalement en construction : porte `lint-i18n-parity`

Nouvelle porte `cargo xtask lint-i18n-parity` : elle aplatit chaque catalogue de
`frontend/ui/src/i18n/locales` en clés pointées (feuilles) et signale, pour chaque locale non-référence,
toute clé **présente dans `en`** mais **absente** — c.-à-d. celle qui tomberait sur le repli. Sortie
non nulle → CI rouge. Les clés **supplémentaires** d'une locale (absentes de la référence) ne sont pas
des trous d'affichage et ne sont **pas** signalées. La porte tourne dans le job `ci`, après `lint-i18n`.
Analyseur pur (`flatten`, `missing_keys`) couvert par des tests unitaires xtask.

C'est le pendant statique du repli : le repli évite l'interface trouée à l'exécution ; la porte rend la
lacune **visible et bloquante** avant livraison (les deux critères d'acceptation).

## Conséquences

- Nouvelle commande xtask + 4 tests unitaires ; étape CI `lint-i18n-parity` ; test frontend de repli ;
  double annotation `@implements REQ-I18N-004` (runtime + parité).
- Réutilise `serde_json` (déjà dépendance de xtask) — **aucune** nouvelle dépendance.
- Contrat implicite : `en.json` est la **source de vérité** des clés ; ajouter une clé impose de la
  fournir dans toutes les locales (sinon CI rouge), une traduction manquante restant lisible via le repli.
