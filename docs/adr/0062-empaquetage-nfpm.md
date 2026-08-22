# ADR 0062 — Empaquetage deb et rpm par nfpm, recette unique

- **Statut** : accepté (2026-08-22)
- **Contexte** : REQ-OPS-010 exige un paquet Debian **et** un paquet RPM de la même version,
  chacun posant binaire, interface compilée, unité de service et configuration par défaut.

## Problème

Deux formats, une seule vérité. Les outils par-format (`cargo-deb` pour le deb,
`cargo-generate-rpm` pour le rpm) imposent **deux** recettes à maintenir en accord — précisément
le genre de dérive silencieuse que ce dépôt s'emploie à rendre impossible. Il faut un point unique
où binaire, actifs, unité, config, scripts et métadonnées sont déclarés une fois.

## Décision

**nfpm** (goreleaser/nfpm) : une recette `packaging/nfpm.yaml`, deux invocations
(`--packager deb`, `--packager rpm`) sur le **même** contenu et la **même** version, injectée par
`WALLOS_VERSION`. nfpm est un binaire autonome utilisé en CI de release
(`.github/workflows/release.yml`) — ce n'est pas une dépendance du code (R6 ne s'applique pas au
sens strict), mais l'outillage de release est une décision d'architecture : cet ADR l'acte.

La conformité de la recette aux critères d'acceptation est verrouillée par
`crates/server/tests/packaging.rs` : tests de garde sur la recette committée (emplacements
standards, config 0640 sans secret prédéfini, `config|noreplace`, compte système dédié,
`try-restart` en mise à jour, purge explicite, aucune dépendance PostgreSQL).

## Conséquences

- L'archive `apk` et d'autres formats nfpm restent accessibles plus tard sans nouvelle recette
  (REQ-OPS-011).
- La signature des artefacts (REQ-OPS-012) se greffera sur la même CI de release.
- Le paquet ne démarre pas le service à l'installation : sans `DATABASE_URL` ni secrets — jamais
  prédéfinis — un démarrage échouerait par construction ; l'exploitant configure
  `/etc/wallos-server/wallos-server.env` puis active. Une mise à jour, elle, redémarre un service
  actif (`try-restart`).
