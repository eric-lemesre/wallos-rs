# ADR 0059 — Oracle exécutable : du relevé lu au comportement observé

- **Statut** : accepté (2026-08-17)
- **Contexte** : question du responsable (2026-08-17) — « le back-end reproduit-il le fonctionnement
  de l'application d'origine ? ». L'audit mené pour y répondre a révélé un écart entre le protocole
  décrit et le protocole appliqué.
- **Complète** : ADR 0011 (cible figée), ADR 0057 (principe frontend).

## Constat

`AGENTS.md` §8.1 décrit un protocole en quatre temps pour toute exigence `oracle: legacy` : écrire le
scénario, **l'exécuter contre l'application d'origine où il doit passer**, geler le résultat, puis le
rejouer contre la nôtre. L'étape 2 est celle qui protège du pire défaut possible — une lecture
erronée du code de référence produisant un oracle faux, contre lequel notre application est ensuite
vérifiée avec succès. Tout serait vert, et rien ne le signalerait.

Cette étape 2 n'a jamais été exécutée.

| Niveau de preuve | Exigences | Détail |
|---|---|---|
| Code d'origine **exécuté** (`docker exec … php -r`, requêtes SQLite) | **9** | STA-001/002/004/005, SUB-007/012/013, NOT-001, CAT-001 |
| Source ou schéma **lus** seulement | **10** | CAT-002/003, CUR-005/007, NOT-003/005, STA-003, SUB-003/010/017 |
| **Rien de capturé** | **14** | SUB-001/002/004/006/008/009/011/014, CUR-001, I18N-001, NOT-004/006, STA-007, CLT-008 |

`LegacyDriver` fait soixante lignes et n'implémente que l'authentification — son propre commentaire
annonçait que les opérations métier viendraient « avec la première exigence `oracle: legacy` », ce
qui n'est pas arrivé. `e2e/legacy/` ne contient qu'un smoke de connexion. Dans `e2e/specs/`, six
scénarios portent le tag `@legacy` contre soixante-neuf `@design`.

## Décision

### Dire ce qui est, avant de décider quoi faire

`AGENTS.md` §8.1 est corrigé pour décrire le protocole **réellement suivi**, avec ses trois niveaux
de preuve nommés et l'étape manquante déclarée. Un contrat qui décrit une pratique inexistante ne
guide plus — c'est le défaut déjà corrigé au §7 (ADR 0057), et il valait ici aussi.

### Une porte plutôt qu'une bonne intention

Nouvelle porte `cargo xtask oracle-coverage` : toute exigence `oracle: legacy` doit disposer d'un
oracle gelé déclarant son **niveau de preuve**. Ce qui manque devient une dérogation **datée et
justifiée**, sur le modèle de `spec/e2e-waivers.yaml` — visible, plutôt que déduit d'un décompte que
personne ne fait.

Le niveau de preuve entre dans l'oracle lui-même (`_evidence` : `executed` | `read` | `observed`).
C'est ce qui permet de distinguer, à la lecture d'un test vert, ce qui est démontré de ce qui est
supposé.

### Rendre le pilote d'origine réel

`LegacyDriver` est étendu au périmètre nécessaire pour **observer** les comportements plutôt que les
déduire. C'est le vrai coût de cette décision : l'application d'origine est du PHP avec ses propres
sélecteurs, et l'interface de pilotage compte quarante-deux méthodes.

L'ordre retenu privilégie le rendement : d'abord les exigences **sans aucune capture**, puis celles
en lecture seule dont la règle est la plus facile à mal lire. Les neuf déjà exécutées ne sont pas
prioritaires — leur niveau de preuve est déjà supérieur à ce qu'une observation d'interface
apporterait.

### Étendre l'oracle à l'interface (REQ-CLT-009)

Pour le back-end, lire le code suffisait souvent : la règle d'arrondi *est* dans le source. Pour
l'interface, non — ce qui compte est ce qui est **rendu** : champs offerts, colonnes, tris, états
vides, libellés. On ne le dérive pas de façon fiable d'un gabarit PHP.

Piloter l'application réelle, facultatif pour le back-end, devient donc **la seule méthode fidèle**
pour l'interface. C'est l'objet de REQ-CLT-009.

## Bornes

**Ce qui est relevable** : l'inventaire des affordances par écran, les styles calculés, les libellés
rendus, le graphe de navigation.

**Ce qui ne l'est pas, et pourquoi** :

- le **rendu au pixel** — DOM et cadriciel diffèrent par construction ; l'oracle ne serait que du
  bruit ;
- la **structure du DOM** — sans signification d'une implémentation à l'autre ;
- la **mise en page** — l'ADR 0055 engage trois modalités ; reproduire une mise en page de bureau
  interdirait la coquille mobile ;
- les **défauts de l'original** — contrastes insuffisants, libellés fautifs, lacunes
  d'accessibilité. Les geler en exigences les importerait. Ils sont **déclarés** dans l'oracle et
  explicitement **non reproduits**.

## Conséquences

- La question « le back-end reproduit-il l'original ? » reçoit enfin une réponse **mesurable** :
  neuf démontrées, dix plausibles, quatorze affirmées. Le décompte devient une porte, pas une
  enquête ponctuelle.
- Le badge d'exigences ne change pas : ces exigences restent `verified` — leurs tests passent, et
  leur implémentation est réelle. C'est le **niveau de preuve de la référence** qui est en cause,
  pas la qualité du code. Confondre les deux serait aussi trompeur que de les taire.
- REQ-CLT-008 est corrigée : son relevé vit dans la feuille de style plutôt que dans un oracle gelé,
  ce qui la rendait invisible au décompte. Incohérence introduite avec elle, réparée avec la porte.
- Coût assumé : étendre `LegacyDriver` est un travail long, sur une base PHP qu'on ne maîtrise pas.
  Il est **borné par le rendement** — les exigences sans capture d'abord, jamais un balayage complet.
