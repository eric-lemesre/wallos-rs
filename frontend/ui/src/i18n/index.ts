import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import fr from "./locales/fr.json";

/**
 * Initialisation i18next. Aucune chaîne littérale d'interface n'est écrite en JSX
 * (REQ-I18N-002) : toutes les libellés proviennent des ressources ci-dessous.
 *
 * @implements REQ-I18N-002
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
