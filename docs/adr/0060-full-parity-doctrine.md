# ADR 0060 — Parité intégrale : reproduire 100 % de l'application d'origine

- **Statut** : accepté (2026-08-17)
- **Contexte** : décision du responsable (2026-08-17), en réponse à OQ-016. L'inventaire figé
  (ADR 0059, `REQ-CLT-009-legacy-surface.json`) mesurait une couverture de **43 %** ; la question
  était de savoir si ce chiffre décrivait une dette ou un choix de périmètre. **C'est une dette.**
- **Révise** : la doctrine de parité elle-même, donc l'interprétation de tous les ADR qui ont acté
  une divergence fonctionnelle.

## Décision

**Toute fonctionnalité de l'application d'origine doit être reproduite.** Développer davantage reste
autorisé — la parité est un **plancher**, jamais un plafond.

### Le critère d'admissibilité d'un écart

Un écart n'est recevable qu'à **deux** conditions, et l'une des deux seulement :

1. **Écart technique** — il découle de la pile, pas d'un choix fonctionnel. L'utilisateur ne peut
   pas l'observer autrement que par la performance ou la robustesse. SQLite devenu Postgres, pages
   rendues côté serveur devenues interface unique sur API JSON, `cron` PHP devenu ordonnanceur Rust,
   configuration en fichier devenue variables d'environnement, tables `cycles`/`frequencies`
   devenues un type énuméré : rien de tout cela ne change ce que l'utilisateur peut faire.

2. **Sur-ensemble strict** — nous faisons plus, et le comportement d'origine reste **exactement**
   obtenable comme cas particulier. La synchronisation multi-appareils, les payeurs, l'idempotence
   de l'ordonnanceur, le chiffrement des secrets au repos n'ôtent rien : ils ajoutent.

**Tout le reste est une régression**, quelle qu'en soit la justification passée — y compris quand
l'écart était mieux conçu que l'original. *Mieux* n'est plus un motif recevable ; *en plus* l'est.

### La preuve fait partie de l'exigence

Une fonctionnalité reproduite sans preuve n'est pas reproduite. Chaque exigence de parité porte :

- un **test unitaire** lorsque la reproduction met en jeu une règle calculable — arrondi, échéance,
  normalisation, tri, agrégat. Le vecteur vient de l'oracle, pas de notre raisonnement ;
- un **test de bout en bout**, sans exception. C'est lui qui atteste que la fonctionnalité est
  *atteignable par l'utilisateur*, et pas seulement présente dans le code.

L'ADR 0059 devient donc contraignant : le niveau de preuve `observed` — scénario rejoué contre
l'application en marche — cesse d'être une cible pour devenir l'objectif de toute exigence de
parité. `LegacyDriver` doit être étendu en conséquence.

## Ce que cette décision remet en cause

Six écarts ont été actés par des ADR antérieurs. Chacun est ici reclassé selon le critère ci-dessus.
Aucun n'est corrigé par cet ADR : chacun appelle son propre lot.

| ADR | Écart | Verdict |
|-----|-------|---------|
| 0012 | Foyer partagé, payeurs = membres, là où Wallos a des comptes individuels | **Sur-ensemble à prouver.** Recevable *si* un foyer d'un seul membre se comporte exactement comme un compte Wallos. Ce n'est aujourd'hui ni vérifié ni testé — à établir, faute de quoi l'écart devient une régression. |
| 0024 | Le tri porte sur le montant **normalisé**, là où Wallos trie le **prix brut** | **Régression.** Le tri de l'original doit être reproduit à l'identique. Le tri normalisé peut subsister **en plus**, comme option. La recherche, absente de Wallos, est un ajout légitime. |
| 0031 | Les entrées de coût nul sont **conservées** dans la répartition, là où Wallos les omet | **Régression.** Comportement observable, à aligner. Le bucket « (aucun) » explicite reste un sur-ensemble recevable. |
| 0023 | Catégories par défaut **traduites**, là où Wallos insère un jeu fixe | **Régression** si l'utilisateur voit des libellés différents de l'original à langue égale. À vérifier contre l'image avant correction. |
| 0025 | Sémantique « actif à ce mois-là » **définie par conception**, faute d'historique d'activation | **À trancher sur pièce.** Si Wallos produit un résultat observable différent, c'est une régression ; sinon, c'est une précision d'implémentation. Aucune capture ne permet aujourd'hui de le dire. |
| SUB-015 | Substitut de logo local, **aucune récupération distante** (choix de confidentialité) | **Régression.** Wallos offre `logos.php`, la recherche d'images et le détourage. La confidentialité justifiait l'écart ; elle ne le justifie plus. Le substitut local reste pertinent **en repli**. |

Le cas de SUB-015 mérite d'être dit franchement : l'écart était **défendable et probablement
meilleur** — ne rien envoyer à un tiers protège l'utilisateur. Il tombe quand même. C'est le prix de
la règle, et une règle qui plie devant le premier bon argument ne règle plus rien.

## Conséquences

- **OQ-016 est résolue** : option B. Les manques deviennent un backlog d'exigences, non un choix de
  périmètre. Les 43 % mesurés sont une dette, et le taux devient un indicateur de progression.
- **Backlog de parité à ouvrir**, par nature et par risque : authentification (réinitialisation de
  mot de passe, vérification d'e-mail, TOTP), exploitation (sauvegarde et restauration),
  fonctionnel (calendrier, export iCal, clonage, budget), canaux (les six restants sur dix),
  administration (comptes multiples, OIDC, inscriptions ouvertes, SMTP global), préférences
  d'affichage, fonctions d'assistance, logos distants.
- **L'ordre reste dicté par le risque, pas par le décompte.** La réinitialisation de mot de passe
  d'abord : un utilisateur qui l'oublie est aujourd'hui enfermé dehors. Puis la sauvegarde et la
  restauration. Le reste ensuite.
- **Le badge d'exigences va monter, puis le taux de couverture aussi** — et les deux ne mesurent
  toujours pas la même chose. Le premier dit ce que nous avons spécifié et vérifié ; le second, ce
  qui manque encore de l'original.
- **Coût assumé** : la parité intégrale importe aussi ce que l'original a de discutable. Là où une
  fonctionnalité d'origine est fautive — contraste insuffisant, message trompeur —, la fonctionnalité
  est reproduite mais le **défaut** ne l'est pas ; l'ADR 0059 avait déjà posé cette distinction pour
  l'interface, elle vaut partout.
