# ADR 0057 — Principe de développement frontend : UI unique, coquilles minces, atelier de composants

- **Statut** : accepté (2026-08-16)
- **Contexte** : demande du responsable (2026-08-16) de **reprendre pour wallos-rs le principe de
  développement frontend éprouvé sur `eric-lemesre/ergonomia`**, dépôt qui sert déjà trois
  modalités (web, bureau, mobile) depuis une base d'interface unique.
- **Complète** : ADR 0055 (retour des clients natifs), ADR 0056 (dépôt unique).

## Problème

L'ADR 0055 fait entrer trois clients dans le périmètre. Le frontend de wallos-rs n'est pas prêt à
les porter, et l'écart n'est pas seulement d'ambition — il est de **structure**, et le contrat
écrit ne le dit pas :

- **La coquille n'est pas mince.** `frontend/shells/web/src/main.tsx` monte **22 composants** à la
  main et les importe par des chemins relatifs `../../../ui/src/…`. AGENTS.md §7 lui impose un
  budget de 300 lignes en la déclarant « montage et configuration » ; en fait, **c'est là que vit
  la composition de l'application**. Ajouter une coquille bureau reviendrait à dupliquer ce
  montage, donc à laisser diverger deux applications.
- **Il n'existe aucune feuille de style.** Zéro fichier CSS dans tout le frontend. Il n'y a ni
  jeton de couleur, ni primitive visuelle, ni règle d'interaction — rien à quoi une revue d'UI
  pourrait se référer.
- **AGENTS.md §7 décrit une architecture absente.** Son tableau de « choix figés » fixe TanStack
  Query, TanStack Router et Zustand : **aucun des trois n'est installé**. Un contrat qui décrit ce
  qui n'existe pas ne guide plus, il induit en erreur.

## Décision

Reprendre le modèle d'`ergonomia`, dont la valeur est d'être **déjà éprouvé** sur trois modalités
par la même équipe, plutôt que d'inventer une organisation propre à wallos-rs.

### 1. Un paquet d'interface, des coquilles de quelques lignes

Toute l'application — routeur, contexte d'authentification, écrans, i18n — vit dans le paquet
d'interface partagé et s'expose par **un seul composant racine** :

```tsx
// frontend/shells/<canal>/src/main.tsx — la coquille entière
createRoot(root).render(
  <StrictMode>
    <App canal="web" apiBaseUrl={apiBaseUrl} />
  </StrictMode>,
);
```

Une coquille se réduit à `index.html`, ce `main.tsx`, un `vite.config.ts`, et — pour les natives —
le projet natif. Le budget de 300 lignes d'AGENTS.md devient un **plafond très large** au lieu
d'une limite qu'on frôle : la coquille d'`ergonomia` fait **douze lignes**.

### 2. La plateforme se réduit à deux paramètres

C'est l'enseignement le plus contre-intuitif du dépôt de référence, et il **corrige** ce que
REQ-CLT-003 avait spécifié. La différence entre web, bureau et mobile n'y passe pas par une
abstraction de capacités : elle tient dans **deux props** — `canal` (un discriminant
`"web" | "desktop" | "mobile"`) et `apiBaseUrl` (vide en web, où l'API est sur la même origine ;
explicite en natif, où le client n'a pas d'origine implicite).

REQ-CLT-003 est donc **simplifiée** : un adaptateur ne sera introduit que lorsqu'une capacité
réellement native l'exigera — magasin de secrets (REQ-CLT-004), notification système
(REQ-NOT-008) —, et pour ces capacités-là seulement. Spécifier d'emblée un adaptateur généralisé
serait construire l'abstraction avant d'avoir les cas qui la justifient.

### 3. Le contrat d'API est un paquet, pas un dossier

Le schéma généré depuis l'OpenAPI devient un **paquet à part entière**, consommé par le paquet
d'interface. La porte de dérive existante (R8) ne change pas ; ce qui change, c'est que le contrat
cesse d'être un détail interne de l'interface pour devenir une dépendance déclarée — ce qui la rend
consommable par une coquille, un test, ou un outil, sans chemin relatif.

L'ensemble est tenu par des **espaces de travail npm** déclarés à la racine, ce qui supprime les
`package-lock.json` par paquet et les dépendances dupliquées entre `ui` et `shells/web`.

### 4. Un design system à feuille unique, piloté par des classes

Une feuille de style unique porte les **jetons** (variables CSS) et les classes des composants. La
règle qui la rend tenable : **un composant n'importe jamais de CSS** — il pose des `className`. La
feuille est importée **une fois** par point d'entrée (chaque coquille, et l'atelier). Motif
technique repris tel quel : un `import "*.css"` ferait échouer le `typecheck` dont la portée est
`src`.

### 5. Storybook est l'atelier, MSW la frontière

Les composants se développent **en isolation**, chaque état étant une *story*. Les écrans **réels**
y sont rendus aussi, en n'interceptant que la **frontière réseau** (MSW) : client, contexte d'auth,
hooks et gardes s'exécutent inchangés, ce qui donne des états *données / vide / erreur* fidèles.

Conséquence de méthode, empruntée à l'« Open Design » d'`ergonomia` : **la story est la maquette**.
Aucune maquette HTML jetable n'est committée — elle se périmerait, une story non.

### 6. La charte d'interaction documente l'existant

Elle est la référence UX, et sa règle d'or est qu'elle **décrit le code**, chaque point renvoyant à
un fichier ou une story réels ; ce qui n'est pas tenu y figure en **lacune assumée**, pas en
silence. Elle sera donc écrite **après** le design system, et non maintenant : rédiger aujourd'hui
une charte pour une interface sans une seule ligne de CSS produirait exactement le défaut
qu'AGENTS.md §7 illustre — un document décrivant ce qui n'existe pas.

## Conséquences

- AGENTS.md §7 est réécrit : il décrivait le natif comme hors périmètre (périmé par l'ADR 0055) et
  figeait trois bibliothèques absentes. Les choix non tenus sont ramenés au rang de **cibles**,
  explicitement distinguées de l'existant.
- REQ-CLT-003 passe d'un adaptateur généralisé à `App({ canal, apiBaseUrl })`. Exigence encore
  `draft` et non implémentée : la corriger maintenant ne coûte rien, la garder coûterait une
  abstraction sans usage.
- Le chantier se mène en **cinq étapes ordonnées**, chacune livrable seule : espaces de travail npm
  → `App` exporté et coquille réduite → paquet de contrat → design system → atelier Storybook. La
  charte d'interaction vient en dernier, quand elle aura quelque chose à décrire.
- Coût assumé : le design system est un travail d'interface réel, sans équivalent dans le legacy —
  Wallos a sa propre feuille de style, qui n'est pas un oracle de **comportement** et ne contraint
  donc pas la parité (§ oracles `legacy`).
