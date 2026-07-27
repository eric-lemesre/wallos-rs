import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Répertoire des oracles gelés (§8.1 step 3) : valeurs observées sur la cible legacy, réinjectées
// ensuite comme fixtures des tests `core` et pilotant la cible `app`.
const ORACLES_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures", "oracles");

/**
 * Gèle une valeur d'oracle si `RECORD` est défini (`pnpm e2e:record`), sinon relit l'oracle gelé.
 * Permet d'écrire un scénario une seule fois : on l'exécute contre `TARGET=legacy` en enregistrant,
 * puis les runs suivants comparent au fichier committé.
 *
 * (Prêt à l'emploi ; non exercé tant qu'aucun scénario `oracle: legacy` n'existe.)
 */
export function recordOracle<T>(name: string, value: T): T {
  const path = join(ORACLES_DIR, `${name}.json`);
  if (process.env.RECORD) {
    mkdirSync(ORACLES_DIR, { recursive: true });
    writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
    return value;
  }
  return JSON.parse(readFileSync(path, "utf-8")) as T;
}
