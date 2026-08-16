import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

/**
 * @verifies REQ-CLT-008
 *
 * Porte du design system. Une couleur écrite en dur dans un écran ne se retrouve plus, ne se
 * corrige plus globalement et rend tout contrôle de contraste illusoire — c'est la dérive que ce
 * test rend impossible, plutôt que déconseillée.
 */

const RACINE = join(__dirname, "..");
const FEUILLE = join(__dirname, "wallos-ux.css");

/** Couleur littérale : hexadécimal, `rgb(...)`, `hsl(...)`. */
const COULEUR_LITTERALE = /#[0-9a-f]{3,8}\b|\brgba?\s*\(|\bhsla?\s*\(/i;

/** Import de feuille de style depuis un composant — interdit (ADR 0057). */
const IMPORT_CSS = /import\s+["'][^"']+\.css["']/;

/**
 * Dérogations, nommées une par une et justifiées — jamais un assouplissement de la règle.
 *
 * `logoSubstitute` **calcule** une teinte par hachage du nom de l'abonnement (REQ-SUB-015) : 360
 * teintes possibles, dérivées d'une donnée utilisateur. Aucun jeu de jetons ne peut les couvrir, et
 * les remplacer par une couleur fixe supprimerait la fonction elle-même — distinguer les
 * abonnements dépourvus de logo.
 */
const DEROGATIONS = new Map([["lib/logoSubstitute.ts", "couleur calculée par hachage (REQ-SUB-015)"]]);

function estDeroge(chemin: string): boolean {
  return [...DEROGATIONS.keys()].some((suffixe) => chemin.endsWith(suffixe));
}

function sourcesTypeScript(dossier: string): string[] {
  return readdirSync(dossier, { withFileTypes: true }).flatMap((entree) => {
    const chemin = join(dossier, entree.name);
    if (entree.isDirectory()) {
      return sourcesTypeScript(chemin);
    }
    return /\.tsx?$/.test(entree.name) && !/\.test\.tsx?$/.test(entree.name) ? [chemin] : [];
  });
}

describe("design system", () => {
  const sources = sourcesTypeScript(RACINE);

  it("balaye effectivement les sources — sans quoi le test passerait à vide", () => {
    expect(sources.length).toBeGreaterThan(20);
  });

  it("n'expose aucune couleur littérale hors de la feuille unique", () => {
    const fautifs = sources
      .filter((f) => !estDeroge(f))
      .filter((f) => COULEUR_LITTERALE.test(readFileSync(f, "utf8")));
    expect(fautifs).toEqual([]);
  });

  it("ne conserve aucune dérogation devenue inutile", () => {
    // Une dérogation qui ne protège plus rien est une permission oubliée : elle rouvrirait la porte
    // sans que personne s'en aperçoive.
    for (const suffixe of DEROGATIONS.keys()) {
      const fichier = sources.find((f) => f.endsWith(suffixe));
      expect(fichier, `dérogation orpheline : ${suffixe}`).toBeDefined();
      expect(COULEUR_LITTERALE.test(readFileSync(fichier!, "utf8"))).toBe(true);
    }
  });

  it("n'importe aucune feuille de style depuis un composant", () => {
    const fautifs = sources.filter((f) => IMPORT_CSS.test(readFileSync(f, "utf8")));
    expect(fautifs).toEqual([]);
  });

  it("redéfinit en thème sombre chaque jeton qui doit y changer", () => {
    const feuille = readFileSync(FEUILLE, "utf8");
    // Trois définitions attendues : clair, `[data-theme="dark"]`, et préférence système.
    for (const jeton of [
      "--background-color",
      "--box-background-color",
      "--box-border-color",
      "--text-color",
      "--text-muted",
      "--input-background-color",
      "--input-border-color",
      "--focus-ring",
    ]) {
      expect(feuille.split(`${jeton}:`).length - 1, `${jeton} manque dans un thème`).toBe(3);
    }
  });

  it("laisse la couleur d'action commune aux deux thèmes, comme la cible figée", () => {
    // Wallos ne redéfinit pas `--main-color` en sombre : le relevé le suit plutôt que d'inventer.
    const feuille = readFileSync(FEUILLE, "utf8");
    expect(feuille.split("--main-color:").length - 1).toBe(1);
  });

  it("ne supprime jamais l'indicateur de focus", () => {
    expect(readFileSync(FEUILLE, "utf8")).not.toMatch(/outline:\s*(none|0)\b/);
  });
});
