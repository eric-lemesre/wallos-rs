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
- **Statut** : open

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
- **Statut** : open

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
- **Statut** : open

---

## OQ-005 — Fournisseur de taux de change
- **Bloque** : REQ-CUR-003
- **Contexte** : les fournisseurs gratuits imposent une clé, un quota, ou disparaissent. Le choix
  conditionne la conception du mode dégradé (REQ-CUR-004).
- **Options** : A) un fournisseur unique configuré par l'utilisateur — B) plusieurs adaptateurs
  derrière un trait, avec repli — C) taux saisis manuellement, sans dépendance réseau
- **Recommandation agent** : B, avec C comme adaptateur de repli toujours disponible. Cela rend
  l'application testable sans réseau, ce qui est une condition de la couverture à 100 %.
- **Statut** : open

---

## OQ-006 — Portée de la modalité mobile en v1
- **Bloque** : niveau L3 de la stratégie E2E
- **Contexte** : la coquille mobile Tauri implique signature, magasins d'applications et
  permissions natives — un coût qui n'est pas couvert par la génération de code.
- **Options** : A) web responsive installable en v1, coquille native reportée — B) coquille
  native dès la v1
- **Recommandation agent** : A. Le rapport effort/valeur de la coquille native est faible tant que
  l'UI partagée n'est pas stabilisée.
- **Statut** : open

---

## OQ-007 — Traitement des exigences `oracle: legacy` non reproductibles
- **Bloque** : protocole §8.1
- **Contexte** : certaines exigences ne pourront pas être capturées sur l'application d'origine
  (comportement non déterministe, fonctionnalité absente, dépendance à un service tiers).
- **Options** : A) les basculer en `oracle: design` avec une décision explicite —
  B) les exclure du périmètre
- **Recommandation agent** : A, avec ADR obligatoire. Basculer sans trace reviendrait à laisser
  l'agent inventer la règle métier, ce que tout le dispositif cherche à empêcher.
- **Statut** : open
