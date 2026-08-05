import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import fr from "./locales/fr.json";

/**
 * Initialisation i18next. Aucune chaîne littérale d'interface n'est écrite en JSX
 * (REQ-I18N-002) : toutes les libellés proviennent des ressources ci-dessous.
 *
 * Repli sur la langue de référence (REQ-I18N-004) : `fallbackLng: "en"` — une clé absente du
 * catalogue de la langue active est résolue sur la valeur **anglaise** (langue de référence), jamais
 * affichée en clé brute (interface non trouée). L'absence est par ailleurs **signalée en construction**
 * par la porte `cargo xtask lint-i18n-parity` (toute clé de référence manquante dans une autre locale
 * fait échouer la CI). `en` étant la référence, son propre catalogue est par construction complet.
 *
 * @implements REQ-I18N-002
 * @implements REQ-I18N-004
 */
void i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    fr: { translation: fr },
  },
  lng: "en",
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
