import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";

/**
 * Devise de référence du foyer (REQ-CUR-001) : la devise dans laquelle tous les agrégats sont
 * exprimés. La modifier recalcule les totaux dans la nouvelle devise **sans altérer les montants
 * saisis**. Isolée par le serveur (§9). S'appuie sur le client généré ; aucune chaîne d'affichage en
 * dur (REQ-I18N-002).
 *
 * @implements REQ-CUR-001
 */
export function ReferenceCurrencySetting() {
  const { t } = useTranslation();
  const [current, setCurrent] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/settings/reference-currency");
    if (response.ok && data) {
      setCurrent(data.currency);
      setDraft(data.currency);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function save() {
    const { data, response } = await api.PUT("/settings/reference-currency", {
      body: { currency: draft },
    });
    if (response.ok && data) {
      setCurrent(data.currency);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }

  return (
    <section data-testid="reference-currency" aria-label={t("referenceCurrency.title")}>
      <h2>{t("referenceCurrency.title")}</h2>

      {current !== null && (
        <p data-testid="reference-currency-current">
          {t("referenceCurrency.current")}: {current}
        </p>
      )}

      <label>
        {t("referenceCurrency.label")}
        <input
          data-testid="reference-currency-input"
          value={draft}
          onChange={(e) => setDraft(e.target.value.toUpperCase())}
        />
      </label>
      <button type="button" data-testid="reference-currency-save" onClick={() => void save()}>
        {t("referenceCurrency.save")}
      </button>

      {failed && (
        <p data-testid="reference-currency-error" role="alert">
          {t("referenceCurrency.error")}
        </p>
      )}
    </section>
  );
}
