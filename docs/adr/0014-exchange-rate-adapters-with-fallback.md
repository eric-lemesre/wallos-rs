# ADR 0014 — Taux de change : adaptateurs multiples derrière un trait, avec repli

## Contexte

`OQ-005` restait ouverte et bloquait `REQ-CUR-003` (récupération des taux de change) ; elle
conditionne aussi la conception de `REQ-CUR-004` (mode dégradé). Les fournisseurs gratuits de taux
imposent une clé, un quota, ou disparaissent : une dépendance réseau dure rendrait l'agrégation
multi-devises non testable hors ligne, ce qui est incompatible avec la porte de couverture 100 %
(AGENTS.md §6).

Options recensées : A) un fournisseur unique configuré par l'utilisateur — B) plusieurs adaptateurs
derrière un trait, avec repli — C) taux saisis manuellement, sans dépendance réseau. Recommandation
agent : B, avec C comme adaptateur de repli toujours disponible.

## Décision

Le responsable du dépôt a arbitré **l'option B**.

- Un **trait** de fournisseur de taux (`RateProvider`) est défini dans `core` (contrat pur, sans
  I/O). Les implémentations concrètes (HTTP vers un ou plusieurs fournisseurs) vivent côté serveur.
- Plusieurs adaptateurs peuvent être configurés ; en cas d'échec, un **repli** en chaîne s'applique.
- Un adaptateur de **saisie manuelle / dernier taux connu** (option C) est **toujours disponible**
  en bout de chaîne : l'application reste fonctionnelle et **testable sans réseau**.

## Conséquences

- `REQ-CUR-003` peut avancer : les taux récupérés sont persistés avec leur **date de validité** et
  leur **source** (traçabilité de l'origine du taux).
- `REQ-CUR-004` (mode dégradé) s'appuie sur ce repli : dernier taux connu + date affichée, et si
  aucun taux n'est connu pour une devise, le montant est **exclu** et l'agrégat **explicitement
  signalé comme incomplet**, jamais silencieusement mis à zéro.
- L'adaptateur de repli manuel rend la couverture 100 % atteignable sans dépendance réseau dans les
  tests (condition §6).
- Les clients HTTP des fournisseurs sont des dépendances nouvelles : introduites au fil de
  l'implémentation, chacune sous ADR (R6) et rattachée à une exigence.
- `OQ-005` distincte de la recommandation ? Non : décision conforme à la recommandation B.

## Liens

- AGENTS.md §0, §6 ; `spec/OPEN-QUESTIONS.md` : OQ-005 (résolue par cet ADR).
- Exigences concernées : REQ-CUR-003, REQ-CUR-004 (dépend de REQ-CUR-003).

## Statut

accepted
