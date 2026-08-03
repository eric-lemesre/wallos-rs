# ADR 0027 — Import / export des données : enveloppe réimportable, ids préservés, catégories fusionnées

- **Statut** : accepté (2026-08-03)
- **Contexte** : REQ-SUB-016 (« import et export des données »), exigence `oracle: design`,
  criticality medium, layer `[core, api, ui]`, dépend de REQ-SUB-001 et REQ-CAT-001.

## Problème

L'acceptance impose deux garanties :

1. Un **export JSON complet réimporté dans un compte vierge** reconstruit un état **identique** à
   l'original, échéances recalculées comprises.
2. Un fichier d'export **issu de l'application d'origine** est importé : abonnements, catégories et
   devises sont créés, et un **rapport liste les lignes rejetées**.

L'exigence est `oracle: design` : le harnais legacy (Wallos) n'est pas rejoué ici (il ne le sera qu'avec
la première exigence `oracle: legacy`). Nous **définissons** donc le format d'échange et le comportement
de l'import ; « issu de l'application d'origine » est la *motivation* (chemin de migration), le format
canonique est celui de subtrack.

## Décision

### Enveloppe unique, réutilisant les requêtes de création

`DataBundle { version, reference_currency?, categories[], payment_methods[], subscriptions[] }` sert à la
fois à l'export **et** à l'import, en réutilisant `Create{Category,PaymentMethod,Subscription}Request`.
Ces requêtes portent déjà un `id` **client optionnel préservé** (REQ-SYN-001) et une validation
**champ par champ** (`into_core`) : l'export émet exactement ce que l'import sait relire.

- **`GET /export`** (`exportData`) sérialise les entités possédées du foyer (§9) + sa devise de
  référence. Aucune donnée dérivée (échéances) n'est stockée dans l'enveloppe : elle est recalculée à
  la lecture.
- **`POST /import`** (`importData`) recrée dans le foyer appelant. Import **tolérant** : chaque ligne
  invalide est **rejetée avec sa raison** (`ImportReport { imported, rejected[] }`), les valides créées.
  Une **version** de format inconnue rejette globalement (`422`).

### Les devises ne sont pas des entités

Le référentiel de devises est **figé** (REQ-CUR-002, aucune table par foyer). « Les devises sont
créées » (critère #2) se traduit donc par une **validation** : une devise (d'abonnement ou de référence)
hors référentiel **rejette la ligne** (elle figure au rapport). C'est le mapping honnête du modèle
subtrack.

### Identifiants préservés, mais **catégories fusionnées par nom**

Les clés primaires de `categories`/`payment_methods`/`subscriptions` sont **globales**. Préserver l'`id`
donne une **identité littérale** lors d'une migration vers une instance vierge (ids libres) ; réimporter
dans un foyer où l'`id` existe déjà est correctement rejeté (`DuplicateId`, idempotence défensive).

Un compte neuf n'est cependant **pas vide** : il porte les **catégories par défaut** (REQ-CAT-002), avec
des identifiants **propres au foyer**. Recréer les catégories à l'identique romprait ces contraintes et,
surtout, les abonnements référençant une catégorie par défaut pointeraient vers un id inexistant dans le
foyer cible. La décision : **fusionner les catégories par nom** (insensible à la casse) — réutiliser la
catégorie existante et **remapper** (`cat_map`) les abonnements qui la référencent. Seules les catégories
réellement nouvelles sont créées (id préservé). Conséquence :

- **Round-trip** : un export réimporté reproduit l'état à l'identique *par valeur* — mêmes abonnements
  (attributs, liaisons catégorie/moyen de paiement, échéances recalculées), mêmes catégories par nom ;
  les catégories par défaut ne sont pas dupliquées.
- **Ré-import dans le même foyer** : catégories fusionnées (rien recréé), moyens de paiement et
  abonnements aux `id` déjà pris rejetés — jamais de doublon silencieux.

## Découpage

- **proto** : `DataBundle`, `ImportReport`, `ImportCounts`, `RejectedRow`, `DATA_BUNDLE_VERSION` ;
  réutilise les DTO de création (validation `into_core` inchangée).
- **server `data`** : handlers `export_data` / `import_data` ; deux opérations couvertes par le trio
  authz owner/other/anon (§9). L'ordre d'import (devise, catégories + moyens de paiement, puis
  abonnements) garantit que les liaisons sont résolubles.
- **ui** : `ImportExportCard` (export téléchargeable + zone lisible ; import + affichage du rapport),
  sans chaîne littérale (REQ-I18N-002).
- **e2e** : `e2e: required` ; un spec `@design` exporte puis importe une enveloppe avec un rejet.

## Conséquences

- Nouvelles opérations `exportData` / `importData` ; couverture API et authz à 100 %.
- **Limites assumées** : les moyens de paiement ne sont pas fusionnés par nom (pas d'unicité de nom) —
  un ré-import à `id` neuf en créerait des doublons ; les payeurs ne sont pas modélisés (SUB-017 en
  draft) et transitent comme UUID libre. Une bascule ultérieure (journal, dédoublonnage des moyens de
  paiement) n'altérerait pas le contrat de l'enveloppe.
