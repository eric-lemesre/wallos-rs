<!-- GÉNÉRÉ par `cargo xtask trace --write` — NE PAS ÉDITER À LA MAIN. -->

# Matrice de traçabilité des exigences

Total : **73** exigences — 65 verified · 8 draft.

Source de vérité : [`spec/requirements.lock.yaml`](requirements.lock.yaml) et [`spec/requirements/`](requirements/). Régénérer avec `cargo xtask trace --write`.

## auth

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-AUT-001` | Création d'un compte utilisateur | high | ✅ verified |
| `REQ-AUT-002` | Authentification par e-mail et mot de passe | high | ✅ verified |
| `REQ-AUT-003` | Politique de mot de passe | medium | ✅ verified |
| `REQ-AUT-004` | Session web par jeton opaque en cookie | high | ✅ verified |
| `REQ-AUT-005` | Jeton d'appareil (API porteur, révocable) | high | ✅ verified |
| `REQ-AUT-006` | Liste et révocation des appareils | high | ✅ verified |
| `REQ-AUT-007` | Changement de mot de passe | high | ✅ verified |
| `REQ-AUT-008` | Limitation du taux de tentatives d'authentification | high | ✅ verified |
| `REQ-AUT-009` | Déconnexion | medium | ✅ verified |

## categories

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-CAT-001` | Gestion des catégories | medium | ✅ verified |
| `REQ-CAT-002` | Catégories par défaut à la création du compte | low | ✅ verified |
| `REQ-CAT-003` | Suppression d'une catégorie référencée | medium | ✅ verified |
| `REQ-CAT-004` | Unicité du nom de catégorie par compte | low | ✅ verified |
| `REQ-CAT-005` | Ordre d'affichage des catégories | low | ✅ verified |

## currencies

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-CUR-001` | Devise de référence du compte | high | ✅ verified |
| `REQ-CUR-002` | Représentation décimale des montants | high | ✅ verified |
| `REQ-CUR-003` | Récupération des taux de change | high | ✅ verified |
| `REQ-CUR-004` | Mode dégradé en cas d'échec du fournisseur de taux | high | ✅ verified |
| `REQ-CUR-005` | Règle d'arrondi | high | ✅ verified |
| `REQ-CUR-006` | Formatage localisé des montants | medium | ⚪ draft |
| `REQ-CUR-007` | Référentiel des devises supportées | medium | ✅ verified |

## i18n

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-I18N-001` | Choix et persistance de la langue | medium | ✅ verified |
| `REQ-I18N-002` | Absence de chaîne littérale dans le code | medium | ✅ verified |
| `REQ-I18N-003` | Formats de date et de nombre localisés | medium | ⚪ draft |
| `REQ-I18N-004` | Repli sur la langue de référence | low | ✅ verified |

## notifications

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-NOT-001` | Rappel avant échéance | high | ✅ verified |
| `REQ-NOT-002` | Idempotence de l'ordonnanceur | high | ⚪ draft |
| `REQ-NOT-003` | Canal e-mail | high | ✅ verified |
| `REQ-NOT-004` | Canaux de messagerie tiers | medium | ✅ verified |
| `REQ-NOT-005` | Webhook générique | medium | ✅ verified |
| `REQ-NOT-006` | Test d'un canal de notification | medium | ⚪ draft |
| `REQ-NOT-007` | Politique de réessai et d'abandon | medium | ⚪ draft |
| `REQ-NOT-008` | Notification native sur desktop et mobile | low | ✅ verified |

## ops

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-OPS-001` | Endpoint de santé | low | ✅ verified |

## security

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-SEC-001` | Isolation stricte des données entre comptes | high | ✅ verified |
| `REQ-SEC-002` | Format d'erreur uniforme sans fuite d'information | high | ✅ verified |
| `REQ-SEC-003` | Journalisation sans secret | high | ✅ verified |
| `REQ-SEC-004` | Chiffrement au repos des secrets de configuration | high | ⚪ draft |
| `REQ-SEC-005` | Protection contre la falsification de requête côté serveur | high | ⚪ draft |
| `REQ-SEC-006` | En-têtes de sécurité et politique de contenu | medium | ✅ verified |

## statistics

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-STA-001` | Normalisation du coût mensuel | high | ✅ verified |
| `REQ-STA-002` | Coût annuel normalisé | high | ✅ verified |
| `REQ-STA-003` | Exclusion des abonnements non actifs des agrégats | high | ✅ verified |
| `REQ-STA-004` | Répartition par catégorie et par payeur | medium | ✅ verified |
| `REQ-STA-005` | Échéancier des prochains paiements | medium | ✅ verified |
| `REQ-STA-006` | Évolution du coût sur douze mois | low | ✅ verified |
| `REQ-STA-007` | Cohérence entre les agrégats et les filtres actifs | medium | ✅ verified |
| `REQ-STA-008` | Détermination des agrégats | high | ✅ verified |

## subscriptions

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-SUB-001` | Modèle de données d'un abonnement | high | ✅ verified |
| `REQ-SUB-002` | Création d'un abonnement | high | ✅ verified |
| `REQ-SUB-003` | Modèle de cycle de facturation | high | ✅ verified |
| `REQ-SUB-004` | Modification d'un abonnement | high | ✅ verified |
| `REQ-SUB-005` | Suppression d'un abonnement | high | ✅ verified |
| `REQ-SUB-006` | Liste des abonnements et filtres | high | ✅ verified |
| `REQ-SUB-007` | Recherche et tri | medium | ✅ verified |
| `REQ-SUB-008` | Abonnement désactivé | medium | ✅ verified |
| `REQ-SUB-009` | Date de fin et annulation programmée | medium | ✅ verified |
| `REQ-SUB-010` | Période d'essai gratuit | medium | ✅ verified |
| `REQ-SUB-011` | Moyen de paiement | low | ✅ verified |
| `REQ-SUB-012` | Calcul de la prochaine échéance pour un cycle mensuel | high | ✅ verified |
| `REQ-SUB-013` | Calcul de la prochaine échéance pour les cycles jour, semaine et année | high | ✅ verified |
| `REQ-SUB-014` | Rattrapage des échéances passées | high | ⚪ draft |
| `REQ-SUB-015` | Logo d'abonnement | low | ✅ verified |
| `REQ-SUB-016` | Import et export des données | medium | ✅ verified |
| `REQ-SUB-017` | Rattachement à un payeur | medium | ✅ verified |

## sync

| ID | Titre | Criticité | Statut |
|----|-------|-----------|--------|
| `REQ-SYN-001` | Horodatage de modification et identifiants stables | high | ✅ verified |
| `REQ-SYN-002` | Pierres tombales | high | ✅ verified |
| `REQ-SYN-003` | Récupération incrémentale par curseur | high | ✅ verified |
| `REQ-SYN-004` | Poussée des modifications locales | high | ✅ verified |
| `REQ-SYN-005` | Résolution de conflit | high | ✅ verified |
| `REQ-SYN-006` | Idempotence des opérations d'écriture | high | ✅ verified |
| `REQ-SYN-007` | Fonctionnement hors ligne | high | ✅ verified |
| `REQ-SYN-008` | Appairage et synchronisation initiale | medium | ✅ verified |

