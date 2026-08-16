# ADR 0055 — Retour des clients natifs dans le périmètre : web, bureau et mobile

- **Statut** : accepté (2026-08-16)
- **Supersède** : ADR 0045 (REQ-NOT-008 hors périmètre) — et **retranche** le volet « pas de coquille
  native » de l'ADR 0054, dont le nettoyage de code reste par ailleurs acquis.
- **Contexte** : décision du responsable (2026-08-16), en réponse à OQ-014 (« que recouvre *client*
  dans la demande de paquets système ? »), qui rouvre OQ-009.

## Problème

Le 2026-08-04, OQ-009 avait sorti le natif du périmètre sur un raisonnement en une ligne : *la cible
est la parité avec l'application d'origine ; or Wallos n'a ni client de bureau ni client mobile ;
donc le natif est hors périmètre.* Trois exigences en avaient été rescopées (NOT-008 déclarée sans
objet, AUT-005 re-cadrée en jeton d'API porteur, SEC-006 réduite au volet CSP web), le code mort
supprimé (`crates/desktop`, `crates/client`) et la règle R7 retirée.

La demande de **paquets d'installation « serveur et clients »** a révélé que cette conclusion ne
correspondait plus à l'intention : le produit vise **trois clients — web, bureau et mobile**.

## Décision

**La prémisse est retirée, pas le raisonnement.** La parité reste la règle pour le **comportement
métier** : les oracles `legacy` demeurent la référence, et une divergence de calcul, de règle de
gestion ou de format observable reste un défaut. Mais la parité **ne commande plus le périmètre des
modalités** : le produit assume d'offrir ce que l'application d'origine n'offre pas.

Conséquence directe : un **domaine `CLT`** est créé, portant sept exigences (`draft`) — adaptateur de
plateforme, instance auto-hébergée configurable, stockage sécurisé du jeton, coquille de bureau,
coquille mobile, confinement des capacités, artefacts d'installation.

### Une seule interface, pas trois

La règle qui structure le domaine : les coquilles **empaquettent l'interface web existante**, elles
ne la réécrivent pas. Tout ce qui dépend de la plateforme transite par un **adaptateur unique**
(REQ-CLT-003), de sorte que le code d'interface ignore sur quoi il s'exécute et que le client web
reste complet grâce à une implémentation par défaut. C'est ce qui rend trois clients soutenables à
un coût de maintenance qui n'est pas triple. La règle **R7 est rétablie** pour l'ancrer : aucune
dépendance de coquille hors de `frontend/shells/`.

### Ce qui est rouvert, et ce qui ne l'est pas

**Rouvert** — **REQ-NOT-008** repasse en `draft` : son premier critère (notification système via
l'adaptateur) redevient exigible, l'ADR 0045 étant supersédée. Le volet in-app déjà livré
(`RemindersCard`) reste acquis et satisfait toujours le second critère.

**Non rouvert** — **REQ-AUT-005** et **REQ-SEC-006** restent `verified`. Leur re-cadrage était juste
*en soi*, indépendamment du natif : un jeton d'API porteur révocable est une capacité serveur
légitime, et la CSP web une exigence web. Ce qui leur avait été retiré ne leur revient pas, mais
devient des exigences **client** à part entière — REQ-CLT-004 pour le stockage du jeton dans le
magasin de secrets du système, REQ-CLT-006 pour le confinement des capacités de la coquille. La
séparation est plus nette que l'état antérieur : l'exigence serveur et l'exigence client ne sont
plus mêlées dans un même bloc.

**Non ressuscité** — `crates/client` (SDK Rust) reste supprimé. Les coquilles consomment l'API par
le **client TypeScript généré**, exactement comme le web ; un second client d'API en Rust n'aurait
pas de consommateur, ce qui était déjà la raison de sa suppression.

### Technologie : différée à l'implémentation

Aucune technologie n'est arrêtée ici. Tauri v2 est le candidat évident — il couvre bureau **et**
mobile depuis une base Rust, et réemploie l'interface React existante — mais l'engagement relève
d'un ADR d'implémentation soumis à R6, avec les alternatives réellement pesées à ce moment-là.

## Conséquences

- Le décompte d'exigences **recule** : 73 `verified` deviennent 72, et le domaine `CLT` ajoute sept
  `draft`. C'est l'effet mécanique d'un périmètre élargi, non une régression de qualité — le badge
  cesse simplement d'annoncer une complétude qui n'est plus vraie.
- **OQ-015** est ouverte : signature et publication des clients natifs (certificats macOS et Windows,
  clé de signature Android — non régénérable —, compte de boutique iOS). Ce sont des comptes, des
  coûts annuels et des secrets à long terme : la décision appartient au responsable.
- Les **niveaux e2e** au-delà du web devront être réintroduits dans AGENTS.md **au moment** de
  l'implémentation, pas avant : définir un harnais avant d'avoir une coquille à piloter figerait des
  choix à l'aveugle.
- L'`ops.md` et le `clients.md` se rejoignent sur REQ-CLT-007 : le paquet d'un **client de bureau**
  n'est pas celui d'un **serveur** — entrée de menu et icônes d'un côté, unité de service et compte
  système de l'autre. Les deux chaînes partagent la publication (REQ-OPS-011) et la signature
  (REQ-OPS-012), pas le contenu.
