import { describe, expect, it } from "vitest";

import { formatAmount, formatDate, formatMonth } from "./format";

/** @implements REQ-CUR-006 */
/** @implements REQ-I18N-003 */

describe("formatAmount", () => {
  it("suit la locale : séparateurs et position du symbole diffèrent, la valeur non", () => {
    const fr = formatAmount("1234.5", "EUR", "fr");
    const en = formatAmount("1234.5", "EUR", "en");
    // fr : « 1 234,50 € » (séparateur d'espace, virgule, symbole après).
    expect(fr).toMatch(/1\s234,50\s*€/u);
    // en : « €1,234.50 » (virgule de milliers, point, symbole avant).
    expect(en).toBe("€1,234.50");
  });

  it("n'affiche aucune décimale pour une devise sans sous-unité", () => {
    expect(formatAmount("1500", "JPY", "en")).toBe("¥1,500");
    expect(formatAmount("1500", "JPY", "fr")).not.toContain(",00");
  });

  it("normalise les décimales de la devise (padding)", () => {
    expect(formatAmount("20", "USD", "en")).toBe("$20.00");
  });

  it("retombe sur la forme brute pour une devise inconnue ou un montant illisible", () => {
    expect(formatAmount("9.99", "???", "en")).toBe("9.99 ???");
    expect(formatAmount("abc", "EUR", "en")).toBe("abc EUR");
  });

  it("refuse les formes non canoniques et préserve les montants hors précision double (revue F6/F7)", () => {
    // Non canonique : jamais interprété silencieusement.
    expect(formatAmount("0x10", "EUR", "en")).toBe("0x10 EUR");
    expect(formatAmount("1e2", "EUR", "en")).toBe("1e2 EUR");
    // Plus de 15 chiffres significatifs : la forme brute préserve la valeur exacte (R4).
    expect(formatAmount("123456789012345678.90", "EUR", "en")).toBe("123456789012345678.90 EUR");
  });

  it("n'affiche rien pour un champ vide (revue F8)", () => {
    expect(formatAmount("", "EUR", "en")).toBe("");
    expect(formatAmount("   ", "EUR", "en")).toBe("");
  });
});

describe("formatDate", () => {
  it("suit la locale active, jamais un format codé en dur", () => {
    expect(formatDate("2026-08-07", "en")).toBe("Aug 7, 2026");
    expect(formatDate("2026-08-07", "fr")).toBe("7 août 2026");
  });

  it("affiche le jour civil attendu quel que soit le fuseau (pas de décalage d'un jour)", () => {
    // La date est reconstruite en LOCAL : même dans un fuseau à l'ouest de Greenwich
    // (interprétation UTC → veille), le jour affiché reste le 1er janvier.
    expect(formatDate("2026-01-01", "en")).toContain("1");
    expect(formatDate("2026-01-01", "en")).toContain("Jan");
    expect(formatDate("2026-12-31", "fr")).toContain("31");
  });

  it("renvoie telle quelle une chaîne non conforme", () => {
    expect(formatDate("pas-une-date", "fr")).toBe("pas-une-date");
    expect(formatDate("", "fr")).toBe("");
  });

  it("ne corrige jamais silencieusement une date invalide (revue F9)", () => {
    expect(formatDate("2026-02-30", "en")).toBe("2026-02-30");
    expect(formatDate("2026-13-01", "en")).toBe("2026-13-01");
  });
});

describe("formatMonth", () => {
  it("localise un mois civil YYYY-MM", () => {
    expect(formatMonth("2026-05", "en")).toBe("May 2026");
    expect(formatMonth("2026-05", "fr")).toBe("mai 2026");
  });

  it("renvoie telle quelle une chaîne non conforme", () => {
    expect(formatMonth("2026", "en")).toBe("2026");
  });
});
