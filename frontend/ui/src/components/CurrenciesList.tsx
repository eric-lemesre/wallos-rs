import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type CurrencyDto = components["schemas"]["CurrencyDto"];

/**
 * Référentiel des devises supportées (REQ-CUR-007) : symbole, code, libellé et nombre de décimales.
 * S'appuie exclusivement sur le client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-CUR-007
 */
export function CurrenciesList() {
  const { t } = useTranslation();
  const [currencies, setCurrencies] = useState<CurrencyDto[]>([]);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/currencies");
    if (response.ok && data) {
      setCurrencies(data);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  return (
    <section data-testid="currencies-list" aria-label={t("currencies.title")}>
      <h2>{t("currencies.title")}</h2>

      {failed && (
        <p data-testid="currencies-error" role="alert">
          {t("currencies.loadError")}
        </p>
      )}

      <ul>
        {currencies.map((currency) => (
          <li key={currency.code} data-testid="currency-row">
            <span data-testid="currency-symbol">{currency.symbol}</span>
            <span data-testid="currency-code">{currency.code}</span>
            <span data-testid="currency-name">{currency.name}</span>
            <span data-testid="currency-decimals">
              {t("currencies.decimals")}: {currency.decimals}
            </span>
          </li>
        ))}
      </ul>
    </section>
  );
}
