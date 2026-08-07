# Questions ouvertes

Un agent qui rencontre une de ces questions sur son chemin **s'arrête** et rend la main.
Il ne tranche jamais de sa propre initiative (AGENTS.md §0).

---

## OQ-001 — Version de référence de l'application d'origine
- **Bloque** : toutes les exigences `oracle: legacy`
- **Contexte** : le protocole d'oracle exige une cible figée. Une mise à jour de l'application
  d'origine en cours de projet invaliderait silencieusement les fixtures.
- **Options** : A) figer un tag Docker précis pour toute la durée du projet — B) suivre la
  dernière version et rejouer l'enregistrement des oracles à chaque montée
- **Recommandation agent** : A. La comparaison n'a de sens que contre une cible immobile.
- **Décision** : A. Cible gelée = Wallos `bellamy/wallos:5.4.2`
  (`@sha256:316f26e13265958e7946ef98ff600516fddc51d698ee98bd1ae1577e5e00789f`), dernière stable
  au 2026-07-25, figée jusqu'à implémentation complète. Voir
  `docs/adr/0011-legacy-reference-wallos-5-4-2.md`.
- **Statut** : resolved

---

## OQ-002 — Périmètre du foyer et des payeurs
- **Bloque** : REQ-SUB-017, REQ-STA-004
- **Contexte** : un « payeur » peut être une simple étiquette sur le compte, ou un véritable
  utilisateur invité disposant de son propre accès. La différence est structurante pour
  REQ-SEC-001 : dans le second cas, l'isolation n'est plus par compte mais par foyer.
- **Options** : A) étiquette sans compte — B) membres invités avec accès en lecture —
  C) foyer partagé avec droits d'écriture
- **Recommandation agent** : A pour la v1. B et C multiplient la surface d'autorisation à tester
  sans bénéfice immédiat.
- **Décision** : C. Le foyer est l'unité de propriété et d'isolation (`household_id` non nullable) ;
  ses membres ont lecture + écriture. Un payeur est un membre du foyer. Voir
  `docs/adr/0012-household-shared-ownership.md`. Tension avec l'oracle legacy signalée (OQ-007).
- **Addendum (2026-08-04, OQ-010)** : pour REQ-SUB-017, un **payeur est une étiquette nominative** (table
  `payers`, sans compte ni login) — le multi-membre avec accès (option C originale) n'est **pas** un
  préalable et reste hors périmètre tant qu'aucune exigence ne l'impose. Le foyer reste l'unité
  d'isolation (mono-utilisateur en pratique). Un ADR SUB-017 formalisera ce modèle payeur.
- **Statut** : resolved

---

## OQ-003 — Base de données serveur
- **Bloque** : REQ-SYN-003, choix des migrations
- **Contexte** : hypothèse H3 non arbitrée. PostgreSQL simplifie la pagination stable et la
  concurrence de l'ordonnanceur ; SQLite simplifie radicalement l'auto-hébergement, qui est
  précisément la promesse de l'application d'origine.
- **Options** : A) PostgreSQL — B) SQLite — C) les deux via une abstraction de repository
- **Recommandation agent** : A si le déploiement cible est un serveur, B si la cible est un
  auto-hébergement domestique minimal. C est à éviter : double surface de test pour un bénéfice
  marginal, et la porte de couverture à 100 % devrait alors couvrir les deux moteurs.
- **Décision** : PostgreSQL côté serveur, SQLite côté client desktop/mobile (confirme H3).
  Séparation structurelle par modalité, pas d'abstraction runtime-swappable. Voir
  `docs/adr/0010-database-postgres-server-sqlite-client.md`.
- **Statut** : resolved

---

## OQ-004 — Période de rétention des pierres tombales
- **Bloque** : REQ-SYN-002
- **Contexte** : détermine la durée maximale pendant laquelle un appareil peut rester hors ligne
  avant d'être contraint à une resynchronisation complète.
- **Options** : A) 30 jours — B) 90 jours — C) rétention illimitée
- **Recommandation agent** : B. Un appareil absent plus de trois mois peut légitimement repartir de zéro.
- **Décision** : A (30 jours) **par défaut, paramétrable côté serveur** (opérateur, jamais
  l'utilisateur final). Au-delà de la fenêtre, resynchronisation complète imposée. Voir
  `docs/adr/0013-tombstone-retention-30d-configurable.md`.
- **Statut** : resolved

---

## OQ-005 — Fournisseur de taux de change
- **Bloque** : REQ-CUR-003
- **Contexte** : les fournisseurs gratuits imposent une clé, un quota, ou disparaissent. Le choix
  conditionne la conception du mode dégradé (REQ-CUR-004).
- **Options** : A) un fournisseur unique configuré par l'utilisateur — B) plusieurs adaptateurs
  derrière un trait, avec repli — C) taux saisis manuellement, sans dépendance réseau
- **Recommandation agent** : B, avec C comme adaptateur de repli toujours disponible. Cela rend
  l'application testable sans réseau, ce qui est une condition de la couverture à 100 %.
- **Décision** : B. Trait `RateProvider` dans `core`, adaptateurs HTTP côté serveur, repli en
  chaîne, adaptateur manuel/dernier taux connu toujours disponible en bout de chaîne. Voir
  `docs/adr/0014-exchange-rate-adapters-with-fallback.md`.
- **Statut** : resolved

---

## OQ-006 — Portée de la modalité mobile en v1
- **Bloque** : niveau L3 de la stratégie E2E
- **Contexte** : la coquille mobile Tauri implique signature, magasins d'applications et
  permissions natives — un coût qui n'est pas couvert par la génération de code.
- **Options** : A) web responsive installable en v1, coquille native reportée — B) coquille
  native dès la v1
- **Recommandation agent** : A. Le rapport effort/valeur de la coquille native est faible tant que
  l'UI partagée n'est pas stabilisée.
- **Décision** : A. PWA responsive en v1 ; coquille native mobile reportée. L3 E2E = émulation de
  viewport (pas de smoke natif Maestro en v1). Voir `docs/adr/0015-mobile-v1-responsive-web.md`.
- **Statut** : resolved

---

## OQ-008 — Volet client natif des jetons d'appareil (secureStore / coquille desktop)
- **Bloque** : le critère « stocké via `PlatformAdapter.secureStore` » de REQ-AUT-005, et la partie
  desktop de REQ-AUT-006/007.
- **Contexte** : le back-end des jetons d'appareil est livrable et testable immédiatement, mais
  `frontend/platform` (`PlatformAdapter`/`SecureStore`), la coquille desktop Tauri et le niveau e2e
  L2 (`tauri-driver`) sont absents. Les bâtir est un chantier distinct (plusieurs ADR, nouveau tier
  e2e) qui n'est pas mûr tant que l'UI partagée ne l'est pas (cf. ADR 0015 sur le report du natif).
- **Options** : A) livrer l'API des jetons d'appareil maintenant (Bearer, révocable) + REQ-AUT-006
  en UI **web** (liste/révocation vérifiable en L1) ; laisser REQ-AUT-005 en `implemented` et
  différer le stockage natif — B) tout bloquer jusqu'à la coquille desktop — C) construire la
  coquille native d'abord.
- **Recommandation agent** : A. Débloque un maximum de valeur testable sans figer prématurément
  l'archi native.
- **Décision** : A (arbitrée par le responsable du dépôt, 2026-07-26). API des jetons d'appareil
  livrée (ADR 0019) ; REQ-AUT-005 = `implemented` ; REQ-AUT-006 = `verified` en modalité web ; le
  stockage natif via `secureStore` et l'e2e L2 desktop sont reportés à l'incrément « coquille
  desktop ».
- **Statut** : resolved

---

## OQ-007 — Traitement des exigences `oracle: legacy` non reproductibles
- **Bloque** : protocole §8.1
- **Contexte** : certaines exigences ne pourront pas être capturées sur l'application d'origine
  (comportement non déterministe, fonctionnalité absente, dépendance à un service tiers).
- **Options** : A) les basculer en `oracle: design` avec une décision explicite —
  B) les exclure du périmètre
- **Recommandation agent** : A, avec ADR obligatoire. Basculer sans trace reviendrait à laisser
  l'agent inventer la règle métier, ce que tout le dispositif cherche à empêcher.
- **Décision** : A. Reclassement en `oracle: design` au cas par cas, via ADR dédié mettant à jour
  `spec/requirements/*.md` + le lock ; jamais de basculement silencieux. Voir
  `docs/adr/0016-legacy-non-reproducible-reclassify-to-design.md`.
- **Statut** : resolved

---

## OQ-009 — Périmètre des coquilles natives (desktop / mobile)
- **Bloque** : le critère « stocké via `PlatformAdapter.secureStore` » de REQ-AUT-005, la partie
  desktop de REQ-AUT-006/007, la partie « capacités Tauri » de REQ-SEC-006, le niveau e2e L2.
- **Contexte** : OQ-006 et OQ-008 avaient **reporté** le natif ; la question restait « quand ». Décision
  stratégique du responsable (2026-08-04) : la cible est la **parité** avec l'application d'origine, et
  **Wallos n'a ni desktop ni mobile natifs**. Le natif sort donc du périmètre de parité — il n'est pas
  reporté, il est **hors périmètre** jusqu'à une éventuelle divergence fonctionnelle ultérieure.
- **Décision** : **pas de coquille native** (ni desktop ni mobile) pour la parité. La modalité mobile
  reste la PWA responsive (confirme OQ-006). **Supersède** le volet « reporté » d'OQ-008 : le stockage
  natif `secureStore` et l'e2e L2 desktop sont **retirés**, non différés.
- **Conséquences à traiter** (suivi ci-dessous, ne pas exécuter sans arbitrage) :
  1. **REQ-AUT-005** : son critère #2 (`secureStore`, jamais `localStorage`) devient sans objet →
     re-cadrer l'exigence en « jeton d'API porteur (Bearer), révocable » (capacité web/API, déjà
     `implemented` et testée au niveau API), ou la `deprecated` si sans usage. Voir OQ-011.
  2. **REQ-SEC-006** : ne conserver que le volet **CSP web** ; retirer le critère « capacités Tauri ».
  3. **Périmètre mort** : crate `crates/desktop` (Tauri), règle R7 (`@tauri-apps` hors `shells/`),
     `frontend/shells/{desktop,mobile}`, `frontend/platform` → à `deprecated`/supprimer par un ADR de
     nettoyage. `crates/client` (SDK Rust) perd son consommateur desktop : à réévaluer.
- **Statut** : resolved (décision) — conséquences 1–3 **à ordonnancer**

---

## OQ-010 — Dépendance circulaire REQ-STA-004 ↔ REQ-SUB-017 (axe payeur)
- **Bloque** : REQ-STA-004, REQ-SUB-017 (toutes deux `oracle: legacy`, en attente de l'oracle Wallos).
- **Contexte** : REQ-STA-004 (« répartition par catégorie **et par payeur** ») a besoin de l'entité
  **payeur** de REQ-SUB-017 pour son second axe ; or REQ-SUB-017 **déclare** `depends_on: [SUB-001,
  STA-004]` et son acceptation (« les statistiques par payeur reflètent le rattachement ») s'appuie sur
  STA-004. La dépendance déclarée est **à l'envers** : le producteur (SUB-017, entité payeur) est marqué
  dépendant du consommateur (STA-004, agrégat). Livrer STA-004 « axe catégorie seul » laisserait son
  critère payeur non satisfait → non `verified` (même impasse qu'AUT-005). Rappel : OQ-002 a tranché
  qu'un payeur est un **membre du foyer** (foyer partagé, lecture+écriture) — le modèle payeur existe
  conceptuellement mais le multi-membre n'est pas encore construit.
- **Options** : A) **inverser la dépendance** dans le lock (SUB-017 → fournit l'entité et le
  rattachement ; STA-004 → en dépend et livre les deux axes), ordre SUB-017 puis STA-004 —
  B) fusionner les deux exigences en une seule livraison — C) livrer STA-004 axe catégorie et créer une
  exigence distincte pour l'axe payeur.
- **Recommandation agent** : A. C'est la correction la plus fidèle à la réalité des dépendances ; les
  deux se feront quand l'oracle legacy sera câblé. Nécessite aussi de clarifier si le **multi-membre du
  foyer** (OQ-002 décision C) est réellement à construire pour SUB-017 ou si un payeur reste une
  étiquette nominative sur le foyer.
- **Décision** : A (responsable, 2026-08-04). **Dépendance inversée** : SUB-017 `depends_on: [SUB-001]`,
  STA-004 `depends_on: [STA-001, CAT-001, SUB-017]` — ordre : SUB-017 puis STA-004, livrés ensemble. Le
  **payeur est une étiquette nominative** (table `payers` : id, household_id, name ; pas de compte/login)
  — révise OQ-002 vers l'option A pour ce périmètre (voir addendum OQ-002). L'**oracle legacy est câblé**
  pour ces deux exigences (1er du projet) : LegacyDriver + conteneur Wallos 5.4.2 en CI, capture de la
  répartition et du comportement de suppression de payeur.
- **Statut** : resolved

---

## OQ-011 — Devenir de REQ-AUT-005 sans coquille native
- **Bloque** : promotion de REQ-AUT-005 (`implemented` → `verified`).
- **Contexte** : conséquence directe d'OQ-009. Le back-end des jetons d'appareil (Bearer, révocable) est
  livré et testé au niveau API (ADR 0019) ; seul le critère #2 (stockage natif `secureStore`) et le
  scénario e2e `@REQ-AUT-005` manquaient — or le natif est désormais hors périmètre.
- **Options** : A) **re-cadrer** REQ-AUT-005 en « jeton d'API porteur, révocable » (retirer le critère
  natif, ajouter un e2e web couvrant l'émission/révocation via API) → devient `verified` —
  B) `deprecated` si les jetons Bearer n'ont pas d'usage en parité (à confirmer vs API Wallos) —
  C) le laisser `implemented` indéfiniment.
- **Recommandation agent** : A si l'API Wallos expose une notion de clé/jeton (parité), sinon B. Éviter
  C (dette de statut permanente). ADR obligatoire (met à jour l'exigence + le lock).
- **Décision** : A (responsable, 2026-08-04). REQ-AUT-005 re-cadré en « jeton d'API porteur, révocable »
  (critère `secureStore` natif retiré), promu `verified`. Voir `docs/adr/0028-aut-005-rescope-api-bearer-token.md`.
- **Statut** : resolved

---

## OQ-012 — `e2e` instable : composition des checks requis
- **Bloque** : la protection de branche (2026-08-04) et donc l'auto-merge de la boucle autonome.
- **Contexte** : en activant la protection de branche avec `ci`+`frontend`+`e2e` requis, la suite `e2e`
  s'est révélée **flaky** (courses de rendu React, surtout webkit : `subscriptions-list`, `language`,
  `subscription-search-sort`). Un check requis **instable** bloque des PR saines sur de faux négatifs et
  paralyse la boucle. `ci`+`frontend` (fmt, clippy, build, tests unitaires, typecheck, drifts) couvrent
  déjà de façon **déterministe** le cas concret des PR rouges (ex. l'incident `fmt`).
- **Décision (agent, réversible)** : checks requis = **`ci` + `frontend`** uniquement. `e2e` reste
  exécuté sur chaque PR (signal visible) mais **non bloquant** tant qu'il n'est pas fiabilisé.
- **Suite à traiter** : stabiliser `e2e` (retries Playwright ciblés, `expect.poll` sur les lectures qui
  courent après le rendu, attente d'états déterministes) **puis** rétablir `e2e` en check requis.
- **Résolution (2026-08-07, demande Eric)** : campagne de stabilisation menée en deux temps.
  (1) Au fil de la session : 7 flakes corrigés en CI (reminder-idempotence, language, import-export,
  exchange, monthly/yearly-cost, localized-dates ICU). (2) Campagne dédiée : la **famille de courses**
  identifiée — un rafraîchissement/fetch unique peut précéder le commit d'une création/mutation
  partie de l'UI — est traitée par une **barrière de persistance centrale**
  (`TargetDriver.awaitSubscriptions(names)` : poll AVEC re-rafraîchissement) appliquée à 13 specs,
  et par des polls avec re-fetch sur toutes les lectures post-mutation. Découverte annexe : le rate
  limiting AUT-008 par IP pollue les runs locaux répétés sur base persistante (purger
  `login_attempts` entre les runs ; sans objet en CI, base fraîche). Validation : suites complètes
  locales (94 tests, chromium+webkit, parallélisme max, **zéro retry** — la CI en a un) : 3 runs
  verts sur 4, flake résiduel ~1/380 exécutions sans retry. `e2e` **rétabli en check requis**
  (`ci`+`frontend`+`e2e`) après merge vert de la PR de stabilisation.
- **Statut** : resolved
