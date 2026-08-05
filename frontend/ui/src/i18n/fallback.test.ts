import i18next from "i18next";
import { describe, expect, it } from "vitest";

import en from "./locales/en.json";
import fr from "./locales/fr.json";

/**
 * Repli sur la langue de référence (REQ-I18N-004) : une clé absente du catalogue de la langue active
 * est résolue sur la valeur **anglaise** (référence), jamais affichée en clé brute. La complétude des
 * catalogues est par ailleurs vérifiée en construction par `cargo xtask lint-i18n-parity`.
 *
 * @implements REQ-I18N-004
 */
describe("Repli i18n sur la langue de référence", () => {
  it("affiche la valeur de référence quand la clé manque dans la langue active", async () => {
    // Catalogue actif volontairement incomplet ; la référence (en) porte la clé.
    const instance = i18next.createInstance();
    await instance.init({
      resources: {
        en: { translation: { only_in_reference: "Reference value" } },
        fr: { translation: {} },
      },
      lng: "fr",
      fallbackLng: "en",
      interpolation: { escapeValue: false },
    });
    // Repli : la valeur anglaise est rendue, pas la clé brute. La clé est synthétique (hors catalogue
    // livré, donc hors des clés typées I18N-002) — cast nécessaire pour ce test isolé.
    expect(instance.t("only_in_reference" as never)).toBe("Reference value");
  });

  it("les catalogues livrés sont complets vis-à-vis de la référence (parité)", () => {
    // Miroir applicatif de la porte lint-i18n-parity : toute clé de en existe dans fr.
    const leaves = (obj: unknown, prefix = ""): string[] =>
      obj && typeof obj === "object"
        ? Object.entries(obj as Record<string, unknown>).flatMap(([k, v]) =>
            leaves(v, prefix ? `${prefix}.${k}` : k),
          )
        : [prefix];
    const missing = leaves(en).filter((k) => !new Set(leaves(fr)).has(k));
    expect(missing).toEqual([]);
  });
});
