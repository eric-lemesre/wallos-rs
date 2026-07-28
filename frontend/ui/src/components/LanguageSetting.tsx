import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";

/** Langues supportées (aligné sur `core::Language` et les ressources i18n). */
const SUPPORTED = ["en", "fr"] as const;

/** Langue système si elle est supportée (acceptance #2), sinon `null`. */
function systemLanguage(): string | null {
  const nav = typeof navigator !== "undefined" ? navigator.language : "";
  const base = nav.split("-")[0];
  return (SUPPORTED as readonly string[]).includes(base) ? base : null;
}

/**
 * Choix et persistance de la langue (REQ-I18N-001). La langue est un **choix serveur** (elle
 * conditionne aussi les notifications) : au chargement, on applique la langue persistée si elle existe,
 * sinon la langue système si elle est supportée. Le choix est enregistré côté serveur et appliqué
 * immédiatement. S'appuie sur le client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-I18N-001
 */
export function LanguageSetting() {
  const { t, i18n } = useTranslation();
  const [current, setCurrent] = useState(i18n.language);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    void (async () => {
      const { data, response } = await api.GET("/settings/language");
      if (response.ok && data) {
        // Langue persistée si choisie, sinon repli sur la langue système supportée.
        const chosen = data.language ?? systemLanguage();
        if (chosen) {
          await i18n.changeLanguage(chosen);
          setCurrent(chosen);
        }
      }
    })();
  }, [i18n]);

  async function choose(code: string) {
    const { data, response } = await api.PUT("/settings/language", {
      body: { language: code },
    });
    if (response.ok && data?.language) {
      await i18n.changeLanguage(data.language);
      setCurrent(data.language);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }

  return (
    <section data-testid="language-setting" aria-label={t("language.title")}>
      <h2>{t("language.title")}</h2>

      <p data-testid="language-current">
        {t("language.current")}: {current}
      </p>

      <select
        data-testid="language-select"
        aria-label={t("language.choose")}
        value={current}
        onChange={(e) => void choose(e.target.value)}
      >
        {SUPPORTED.map((l) => (
          <option key={l} value={l}>
            {t(`language.names.${l}`)}
          </option>
        ))}
      </select>

      {failed && (
        <p data-testid="language-error" role="alert">
          {t("language.error")}
        </p>
      )}
    </section>
  );
}
