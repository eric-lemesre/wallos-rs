# Harnais legacy — Wallos 5.4.2 (cible oracle de référence)

Fondation pour le protocole d'oracle (AGENTS.md §8.1) : exécuter les scénarios `oracle: legacy`
contre l'application d'origine **Wallos**, figée à `5.4.2` par **digest** (ADR 0011).

## Contenu
- `docker-compose.yml` — service `wallos` épinglé par digest (source unique du pin). Base éphémère.
- `smoke.spec.ts` — smoke de fondation : Wallos démarre + le `LegacyDriver` s'authentifie.
- Driver : `../drivers/LegacyDriver.ts` ; fabrique : `../drivers/Harness.ts` (`TARGET=legacy|app`).
- Config : `../playwright.legacy.config.ts` (isolée de la cible `app`).
- Oracles gelés : `../fixtures/oracles/` (helper `../drivers/oracle.ts`).

## Lancer
```bash
cd e2e
npm run e2e:legacy      # démarre Wallos (docker) + exécute les specs legacy (chromium + webkit)
npm run e2e:record      # idem en gelant les oracles (RECORD=1)
docker compose -f legacy/docker-compose.yml down   # arrêter Wallos
```

Wallos écoute sur http://localhost:8282. Première visite → inscription, puis login (par **nom
d'utilisateur**). Le câblage CI du harnais legacy et les scénarios métier viendront avec la première
exigence `oracle: legacy`.
