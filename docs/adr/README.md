# Index des décisions d'architecture (ADR)

Registre des décisions engageantes. Chaque ADR fait autorité une fois `accepté` ; toute évolution
passe par un nouvel ADR (jamais une modification silencieuse).

## Priorité de relecture

Colonne **Relecture** — pour orienter l'attention du responsable du dépôt :

- 🔴 **Prioritaire** — encode un **jugement métier ou une interprétation** (typiquement les exigences
  `oracle: design`, où l'agent a *choisi* la règle). C'est là que se cachent les écarts possibles avec
  l'intention. À relire en premier.
- 🟡 **Utile** — décision structurante déjà cadrée par une question ouverte (`OQ-*`) ; relire si le
  contexte évolue.
- ⚪ **Mécanique** — outillage, process, justification de dépendance ; relecture ponctuelle.

| ADR | Sujet | Relecture |
|-----|-------|-----------|
| 0001 | Intégrité via `xtask` | ⚪ |
| 0002 | Justification des dépendances | ⚪ |
| 0003 | Portée de `lint-money` | ⚪ |
| 0004 | Contrat OpenAPI vide en amorçage | ⚪ |
| 0005 | Code devise en `&str` | 🟡 |
| 0006 | Garde `Actor` (isolation §9) | 🟡 |
| 0007 | Interdiction `Co-authored-by` (R0) | ⚪ |
| 0008 | `xtask` dépend de `server` | ⚪ |
| 0009 | Client frontend généré depuis l'OpenAPI | ⚪ |
| 0010 | Postgres serveur / SQLite client | 🟡 |
| 0011 | Cible legacy figée Wallos 5.4.2 | 🟡 |
| 0012 | Foyer = propriété partagée (payeurs = membres) | 🔴 |
| 0013 | Rétention des pierres tombales (30 j) | 🟡 |
| 0014 | Adaptateurs de taux avec repli | 🟡 |
| 0015 | Mobile v1 = web responsive | 🟡 |
| 0016 | Reclassement `legacy` → `design` | 🟡 |
| 0017 | Pile runtime frontend | ⚪ |
| 0018 | Empreinte SHA-256 des jetons de session | 🟡 |
| 0019 | Jeton d'appareil (Bearer) | 🟡 |
| 0020 | Ignore RUSTSEC-2024-0436 (`paste`) | ⚪ |
| 0021 | Exclusions TRC-06 | ⚪ |
| 0022 | Échéance mensuelle : ancrage + clamp | 🔴 |
| 0023 | Catégories par défaut traduites (seed) | 🔴 |
| 0024 | SUB-007 recherche/tri (divergence legacy) | 🔴 |
| 0025 | STA-006 évolution du coût (« actif ce mois-là ») | 🔴 |
| 0026 | I18N-002 porte anti-chaîne littérale | 🟡 |
| 0027 | SUB-016 import/export (fusion catégories, devises validées) | 🔴 |
| 0028 | AUT-005 re-cadré en jeton d'API porteur (natif retiré) | 🟡 |
| 0029 | Ignore RUSTSEC-2026-0235 (`rkyv` non compilé) | ⚪ |
| 0030 | SUB-017 payeur (étiquette légère, refus si référencé) | 🔴 |
| 0031 | STA-004 répartition (somme conservée, bucket « (aucun) ») | 🔴 |
| 0032 | SEC-006 en-têtes de sécurité + CSP web (natif hors périmètre) | 🟡 |
| 0033 | I18N-004 repli langue (fallbackLng + porte de parité) | ⚪ |
| 0034 | SYN-002 pierres tombales (curseur since, purge, resync) | 🟡 |
| 0035 | SYN-003 delta incrémental (keyset, payload = ligne stockée) | 🟡 |
| 0036 | SYN-004 push (lot indépendant, idempotent, rejets par entité) | ⚪ |
| 0037 | SYN-005 conflit (LWW + concurrence optimiste + journal) | 🔴 |
| 0038 | SYN-007 hors ligne (outbox local, synchro auto, sans natif) | 🟡 |
| 0039 | SYN-008 appairage + synchro initiale reprenable (curseur) | ⚪ |
| 0040 | NOT-001 rappels (cron + endpoint, déclenchement exact, regroupement) | 🔴 |
| 0041 | SUB-010 essai gratuit (design — absent de Wallos) | 🔴 |
| 0042 | STA-003 exclusion transverse (essai exclu par occurrence dans l'échéancier) | 🟡 |
| 0043 | NOT-005 webhook (abstraction fermée, garde SSRF à l'enregistrement, cycle SEC-005) | 🔴 |
| 0044 | NOT-003 e-mail (dépendance `lettre`, destinataire=compte, corps localisé serveur) | 🟡 |
| 0045 | NOT-008 natif hors périmètre (OQ-009) — **supersédée par 0055** | ⚪ |
| 0046 | NOT-004 messageries : quatre adaptateurs sur l'abstraction de canal, divergences legacy assumées | 🔴 |
| 0047 | NOT-006 test d'un canal : envoi sur le canal enregistré, diagnostic par code stable | 🟡 |
| 0048 | NOT-002 idempotence : unicité en base, pas de verrou, pas de rattrapage | 🔴 |
| 0049 | NOT-007 réessai : outbox par (canal, lot), intervalle croissant borné, abandon visible | 🟡 |
| 0050 | SUB-014 rattrapage des échéances : convergence par calcul, jamais par rejeu | 🔴 |
| 0051 | CUR-006 / I18N-003 formatage localisé : `Intl` natif, locale = langue de l'interface | 🟡 |
| 0052 | SEC-005 SSRF : validation des adresses résolues à la connexion, redirections refusées | 🟡 |
| 0053 | SEC-004 secrets au repos : AES-256-GCM applicatif, clé dérivée d'`ENCRYPTION_KEY` | 🟡 |
| 0054 | Nettoyage du périmètre natif (exécution des conséquences d'OQ-009) | ⚪ |
| 0055 | **Retour des clients natifs** : web + bureau + mobile, domaine `CLT`, R7 rétablie | 🔴 |
| 0056 | **Dépôt unique** : serveur, interface, coquilles et paquets dans `wallos-rs` (R9) | 🟡 |
| 0057 | **Principe frontend** repris d'`ergonomia` : `App({canal, apiBaseUrl})`, coquilles minces, design system à feuille unique, Storybook + MSW | 🔴 |
| 0058 | Atelier : Storybook, et MSW à la **seule** frontière réseau (dépendances de dev, R6) | 🟡 |
| 0059 | **Oracle exécutable** : niveaux de preuve, porte `oracle-coverage`, relevé d'interface | 🔴 |
| 0060 | **Parité intégrale** : reproduire 100 % de l'original ; seuls écarts recevables = technique ou sur-ensemble strict (R10/R11) | 🔴 |

## À relire en priorité aujourd'hui (🔴)

Les décisions d'interprétation prises jusqu'ici, susceptibles de diverger de l'intention :

- **0027** — l'import **fusionne les catégories par nom** et ne « crée » pas les devises (validées
  seulement) ; à confronter au format réel d'export Wallos quand la migration sera testée.
- **0024** — la recherche texte et le tri divergent **volontairement** du legacy (Wallos n'a pas de
  recherche ; trie le prix brut, pas le montant normalisé).
- **0025** — sémantique « abonnement actif à ce mois-là » définie par conception (pas d'historique
  d'activation dans le modèle).
- **0023** — jeu de catégories par défaut (impacte tout import/export et l'expérience initiale).
- **0060** — **la parité devient intégrale** (décision du 2026-08-17). Reclasse six écarts déjà
  actés : **quatre régressions** à corriger (tri normalisé 0024, entrées de coût nul 0031, catégories
  traduites 0023, logos distants SUB-015), **un sur-ensemble à prouver** (foyer 0012), **un à trancher
  sur pièce** (0025). À relire en premier : cet ADR change la lecture de tous les autres.
- **0059** — **la parité n'est démontrée que pour un tiers du périmètre** : sur 33 exigences
  `oracle: legacy`, 9 reposent sur l'exécution du code d'origine, 10 sur sa simple lecture, et 14 sur
  rien. Aucun scénario n'a jamais été rejoué contre l'application en marche. À relire avant toute
  affirmation de conformité.
- **0057** — **principe frontend** repris d'`ergonomia`. Corrige un écart de fond : la coquille web
  monte 22 composants (donc *elle* est l'application), il n'existe aucune feuille de style, et
  AGENTS.md §7 figeait trois bibliothèques jamais installées. **Simplifie REQ-CLT-003** :
  `App({canal, apiBaseUrl})` au lieu d'un adaptateur de plateforme généralisé.
- **0055** — **retour du natif dans le périmètre** : renverse la décision d'OQ-009 du 2026-08-04. La
  parité continue de régir le comportement métier, mais plus le périmètre des modalités. À relire en
  premier : c'est la décision la plus structurante prise depuis le cadrage initial.
- **0022** — règle de clamp des échéances mensuelles (28/29/30/31).
- **0012** — foyer partagé, payeurs = membres : structurant pour SUB-017/STA-004 et l'isolation.
- **0040** — rappels : déclenchement **exact** (jour J−N, pas la fenêtre) et regroupement par compte
  capturés sur Wallos ; émission « enregistrée » seulement (canaux réels = NOT-003+, différés).
- **0037** — résolution de conflit : la détection dépend de `base_version` fournie par le client ; sans
  elle, LWW s'applique mais sans journal (écrasement silencieux possible). Compromis option A.
- **0031** — répartition : bucket « (aucun) » explicite (subtrack rend l'axe nullable, là où Wallos a
  des sentinelles) ; petite divergence d'affichage (entrées de coût nul conservées, pas omises).

> Voir aussi `spec/OPEN-QUESTIONS.md` pour les arbitrages **encore ouverts** — à ce jour **OQ-015**
> (signature et publication des clients natifs : certificats macOS et Windows, clé de signature
> Android non régénérable, compte de boutique iOS).
