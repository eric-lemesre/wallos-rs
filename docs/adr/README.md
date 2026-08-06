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

## À relire en priorité aujourd'hui (🔴)

Les décisions d'interprétation prises jusqu'ici, susceptibles de diverger de l'intention :

- **0027** — l'import **fusionne les catégories par nom** et ne « crée » pas les devises (validées
  seulement) ; à confronter au format réel d'export Wallos quand la migration sera testée.
- **0024** — la recherche texte et le tri divergent **volontairement** du legacy (Wallos n'a pas de
  recherche ; trie le prix brut, pas le montant normalisé).
- **0025** — sémantique « abonnement actif à ce mois-là » définie par conception (pas d'historique
  d'activation dans le modèle).
- **0023** — jeu de catégories par défaut (impacte tout import/export et l'expérience initiale).
- **0022** — règle de clamp des échéances mensuelles (28/29/30/31).
- **0012** — foyer partagé, payeurs = membres : structurant pour SUB-017/STA-004 et l'isolation.
- **0037** — résolution de conflit : la détection dépend de `base_version` fournie par le client ; sans
  elle, LWW s'applique mais sans journal (écrasement silencieux possible). Compromis option A.
- **0031** — répartition : bucket « (aucun) » explicite (subtrack rend l'axe nullable, là où Wallos a
  des sentinelles) ; petite divergence d'affichage (entrées de coût nul conservées, pas omises).

> Voir aussi `spec/OPEN-QUESTIONS.md` pour les arbitrages **encore ouverts** (OQ-010 dépendance
> STA-004↔SUB-017, OQ-011 devenir d'AUT-005).
