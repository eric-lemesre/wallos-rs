# ADR 0032 — En-têtes de sécurité + CSP web (SEC-006 réduit au volet web, natif hors périmètre)

- **Statut** : accepté (2026-08-05)
- **Contexte** : REQ-SEC-006 (« en-têtes de sécurité et politique de contenu »), `oracle: design`,
  criticality medium, layer `[api, ui]`, e2e optional, sans dépendance.

## Problème

Deux critères d'acceptation : (#1) une réponse de la **modalité web** porte une **CSP sans directive
permissive de script en ligne** ; (#2) la **configuration Tauri** n'accorde que les capacités
effectivement utilisées, chacune justifiée. Le critère #2 suppose une coquille native.

## Décision

### Critère #2 (capacités Tauri) : sans objet (OQ-009)

Le volet **natif est hors périmètre** (OQ-009 : le legacy n'a ni desktop ni mobile ; cf. ADR 0028 qui a
retiré la coquille Tauri du périmètre AUT-005). Il n'existe **aucune** configuration Tauri à durcir : le
critère #2 est **vacui­tairement satisfait**. SEC-006 est donc **réduit au CSP web** (critère #1) — même
logique de re-cadrage qu'AUT-005.

### Critère #1 (CSP web) : en-tête HTTP serveur + réplique au build

La CSP est délivrée à **deux endroits complémentaires** :

1. **Couche `api` — en-tête HTTP** (`crates/server/src/security.rs`). Un middleware `map_response`
   (axum natif, **aucune dépendance nouvelle**) ajoute à **toute** réponse du serveur :
   `Content-Security-Policy`, `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
   `Referrer-Policy: no-referrer`. L'en-tête HTTP est la **source d'autorité** (prime sur une balise
   `<meta>`). La CSP pose `script-src 'self'` — **jamais** `'unsafe-inline'` ni `'unsafe-eval'`
   (critère #1) ; `style-src 'self' 'unsafe-inline'` reste toléré (attributs `style` en ligne des
   composants React ; le critère ne restreint que le **script**). `frame-ancestors 'none'` +
   `object-src 'none'` + `base-uri 'self'` complètent le durcissement.

2. **Couche `ui` — balise `<meta>` au build** (`frontend/shells/web/vite.config.ts`). Un plugin
   `transformIndexHtml` **appliqué au build uniquement** (`apply: "build"`) injecte la même CSP dans le
   document de **production**. Le confinement au build est **délibéré** : en dev/e2e, Vite sert l'index
   **sans** CSP pour ne pas bloquer son client HMR (scripts en ligne, `eval`, WebSocket). La politique de
   la meta et celle de l'en-tête sont identiques.

### Portée de l'en-tête sur l'API

Le serveur n'expose aujourd'hui que l'API (`/api/v1`) ; la coquille web est servie par Vite (dev/e2e)
ou un hôte statique (prod). Appliquer les en-têtes à **toutes** les réponses du serveur est une **défense
en profondeur** sans effet de bord : une CSP portée par une réponse **JSON** (récupérée par `fetch`)
n'affecte pas la CSP du **document** appelant. Si le serveur venait à servir la SPA, la protection est
déjà en place.

## Conséquences

- Nouveau module `server::security` + 3 tests d'intégration (`#[verifies(REQ-SEC-006)]`) : CSP présente
  avec `script-src` strict (sans `unsafe-inline`/`unsafe-eval`), en-têtes de durcissement, application au
  repli 404.
- `frontend/shells/web/dist/` ajouté au `.gitignore` (sortie de build Vite).
- e2e non requis (optional) ; la meta de build est vérifiée par inspection de `dist/index.html`.
- Si un futur besoin réintroduit une coquille native, un nouvel ADR traitera le critère #2 (capacités).
