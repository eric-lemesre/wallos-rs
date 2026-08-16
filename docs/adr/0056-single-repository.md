# ADR 0056 — Dépôt unique : serveur, interface, coquilles et paquets dans `wallos-rs`

- **Statut** : accepté (2026-08-16)
- **Contexte** : décision du responsable (2026-08-16), à la suite de l'ADR 0055 qui fait entrer trois
  clients dans le périmètre et des REQ-OPS-007/010/011/012 qui introduisent des chaînes de paquets.

## Problème

Élargir le périmètre à trois clients et à quatre familles d'artefacts d'installation crée une
tentation naturelle : sortir la coquille de bureau dans `wallos-rs-desktop`, la coquille mobile dans
`wallos-rs-mobile`, les recettes de paquets dans `wallos-rs-packaging`. C'est le réflexe le plus
courant, et il est ici **destructeur** — pour des raisons propres à ce projet, pas par principe.

## Décision

**Tout vit dans `wallos-rs`.** Aucun dépôt satellite, aucun sous-module. Les coquilles sous
`frontend/shells/{web,desktop,mobile}`, les recettes d'empaquetage sous `packaging/`. La règle **R9**
l'inscrit dans le contrat.

### Pourquoi le dépôt unique n'est pas une préférence ici, mais une condition

1. **Les portes de qualité perdraient leur objet.** R1 exige que *toute* ligne de production soit
   rattachée à une exigence, et `cargo xtask trace` le vérifie en parcourant l'arbre. Du code de
   coquille dans un autre dépôt serait, mécaniquement, du code hors traçabilité — la porte
   resterait verte en ne voyant rien. Le projet perdrait précisément ce qui le définit.

2. **Le contrat d'API est vérifié par un écart, pas par un accord.** Le client TypeScript est
   **généré** depuis `api/openapi.json`, et R8 fait échouer la CI sur toute divergence. Cette porte
   ne fonctionne que si le générateur et le généré sont commités ensemble. Répartis sur deux dépôts,
   ils dérivent entre deux synchronisations, et l'écart n'est plus détecté au moment où il naît mais
   découvert plus tard, en production.

3. **REQ-CLT-007 impose une version commune** au serveur et aux clients. Une version unique se
   constate par un `git describe` ; elle se *négocie* dès qu'il y a deux historiques. La
   vérification de compatibilité client/serveur qu'exige la même exigence deviendrait un protocole
   à maintenir au lieu d'une propriété du dépôt.

4. **Les scénarios e2e sont agnostiques par construction** : les mêmes specs sont rejouées contre
   `LegacyDriver` et `TargetDriver`. Un pilote pour coquille native est un troisième pilote sur les
   **mêmes** scénarios ; le séparer imposerait de dupliquer les specs ou de versionner un paquet de
   specs partagé — deux façons de laisser les comportements diverger en silence.

5. **Une exigence traverse les couches.** REQ-CLT-004 touche l'authentification serveur, l'interface
   et la coquille ; REQ-NOT-008 touche l'ordonnanceur, l'interface et la notification système. La
   règle « un commit = une exigence » suppose qu'un commit puisse les atteindre toutes. Sur
   plusieurs dépôts, chaque exigence transverse se fragmente en commits coordonnés — et la revue
   perd la seule unité qui a du sens.

### Ce que la décision ne dit pas

Elle ne prescrit **ni** un artefact unique, **ni** une chaîne de construction unique : le serveur, le
client de bureau et le client mobile se construisent et se publient séparément, comme le veulent
REQ-OPS-011 et REQ-CLT-007. Un dépôt unique n'est pas un binaire unique.

Elle n'interdit pas non plus qu'un canal de distribution **tiers** (recette communautaire, paquet
maintenu par un empaqueteur de distribution) vive ailleurs : REQ-OPS-011 prévoit explicitement ce
cas, à condition qu'il consomme les artefacts publiés sans modifier la chaîne de construction. La
règle porte sur **ce que le projet maintient**, pas sur ce que d'autres en font.

## Conséquences

- `frontend/shells/{desktop,mobile}` et `packaging/` sont déclarés dans l'arborescence d'AGENTS.md
  avant d'exister : c'est leur emplacement qui est arrêté, pas leur contenu.
- La règle **R9** est ajoutée au contrat, vérifiée en revue.
- L'outillage des coquilles et des paquets devra rester **hors** du chemin critique de la CI serveur :
  un dépôt unique ne doit pas signifier qu'une modification de recette d'empaquetage recompile et
  rejoue l'ensemble. La segmentation des workflows est un sujet d'implémentation, pas de périmètre.
- Le risque assumé est la **croissance du dépôt** (chaînes de construction natives, artefacts
  volumineux). Il se traite par l'exclusion des artefacts du suivi de version, jamais par la scission.
