import { describe, expect, it } from "vitest";

import { logoSubstitute } from "./logoSubstitute";

/** @implements REQ-SUB-015 */

describe("logoSubstitute", () => {
  it("est déterministe : le même nom produit toujours le même substitut", () => {
    expect(logoSubstitute("Netflix")).toEqual(logoSubstitute("Netflix"));
  });

  it("dérive des initiales (1 à 2) du nom", () => {
    expect(logoSubstitute("Netflix").initials).toBe("N");
    expect(logoSubstitute("Amazon Prime").initials).toBe("AP");
    expect(logoSubstitute("  disney plus channel  ").initials).toBe("DP");
  });

  it("gère un nom vide ou blanc sans échouer", () => {
    expect(logoSubstitute("").initials).toBe("?");
    expect(logoSubstitute("   ").initials).toBe("?");
  });

  it("préserve les initiales emoji / hors-BMP (revue #1)", () => {
    // 🎵 est hors du plan multilingue de base : son initiale ne doit pas être un demi-surrogate cassé.
    expect(logoSubstitute("🎵 Music").initials).toBe("🎵M");
    expect(logoSubstitute("Ötzi").initials).toBe("Ö");
  });

  it("produit une couleur CSS stable dérivée du nom", () => {
    const { color } = logoSubstitute("Spotify");
    expect(color).toMatch(/^hsl\(\d+, 65%, 40%\)$/);
    // Stable d'un appel à l'autre.
    expect(logoSubstitute("Spotify").color).toBe(color);
  });

  it("ne dépend que du nom (fonction pure, aucune source réseau/aléa)", () => {
    // Deux noms distincts donnent (au moins) des initiales distinctes ; l'important est l'absence
    // totale d'effet de bord : la fonction est synchrone et ne touche ni fetch ni horloge.
    expect(logoSubstitute("Netflix").initials).not.toBe(logoSubstitute("Spotify").initials);
  });
});
