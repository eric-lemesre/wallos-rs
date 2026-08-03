// Typage des clés de traduction (REQ-I18N-002, critère 2 : « une clé absente du catalogue de
// référence → la construction échoue »).
//
// En augmentant `CustomTypeOptions` avec le catalogue anglais comme référence, `t("…")` n'accepte
// plus que des clés existantes : `tsc --noEmit` (porte `typecheck` en CI) échoue sur toute clé
// inconnue. Le catalogue anglais fait foi ; les autres langues (fr) en sont des traductions.
//
// @implements REQ-I18N-002
import "i18next";

import type en from "./locales/en.json";

declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: {
      translation: typeof en;
    };
  }
}
