import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type ConvertedTotalResponse = components["schemas"]["ConvertedTotalResponse"];
type MoneyInput = components["schemas"]["MoneyInput"];

/**
 * Agrégation multi-devises en **mode dégradé** (REQ-CUR-004) : convertit des montants saisis vers une
 * devise cible en s'appuyant sur les derniers taux connus. L'écran affiche le total, sa **fraîcheur**
 * (date des taux utilisés) et signale **explicitement** un agrégat partiel — jamais un zéro silencieux.
 *
 * Montants transmis en **chaîne** (jamais un nombre JSON, règle R4 / REQ-CUR-002). S'appuie
 * exclusivement sur le client généré ; aucune chaîne littérale en JSX (REQ-I18N-002).
 *
 * @implements REQ-CUR-004
 */
export function ConvertedTotalCard() {
  const { t } = useTranslation();
  const [target, setTarget] = useState("EUR");
  const [rows, setRows] = useState<MoneyInput[]>([{ amount: "", currency: "" }]);
  const [result, setResult] = useState<ConvertedTotalResponse | null>(null);
  const [failed, setFailed] = useState(false);

  function updateRow(index: number, patch: Partial<MoneyInput>) {
    setRows((prev) =>
      prev.map((row, i) => (i === index ? { ...row, ...patch } : row)),
    );
  }

  function addRow() {
    setRows((prev) => [...prev, { amount: "", currency: "" }]);
  }

  async function compute() {
    const { data, response } = await api.POST("/exchange/aggregate", {
      body: { target, amounts: rows },
    });
    if (response.ok && data) {
      setResult(data);
      setFailed(false);
    } else {
      setResult(null);
      setFailed(true);
    }
  }

  return (
    <section data-testid="exchange-aggregate" aria-label={t("exchange.title")}>
      <h2>{t("exchange.title")}</h2>

      <label>
        {t("exchange.target")}
        <input
          data-testid="exchange-target"
          value={target}
          onChange={(e) => setTarget(e.target.value)}
        />
      </label>

      <ul>
        {rows.map((row, index) => (
          // Les lignes sont positionnelles et sans identifiant métier : l'index est une clé stable ici.
          // eslint-disable-next-line react/no-array-index-key
          <li key={index} data-testid="exchange-amount-row">
            <label>
              {t("exchange.amount")}
              <input
                data-testid={`exchange-amount-${index}`}
                value={row.amount}
                onChange={(e) => updateRow(index, { amount: e.target.value })}
              />
            </label>
            <label>
              {t("exchange.currency")}
              <input
                data-testid={`exchange-currency-${index}`}
                value={row.currency}
                onChange={(e) => updateRow(index, { currency: e.target.value })}
              />
            </label>
          </li>
        ))}
      </ul>

      <button type="button" data-testid="exchange-add" onClick={addRow}>
        {t("exchange.addAmount")}
      </button>
      <button type="button" data-testid="exchange-submit" onClick={() => void compute()}>
        {t("exchange.compute")}
      </button>

      {failed && (
        <p data-testid="exchange-error" role="alert">
          {t("exchange.error")}
        </p>
      )}

      {result && (
        <div data-testid="exchange-result">
          <p data-testid="exchange-total">
            {t("exchange.total")}: {result.total} {result.currency}
          </p>
          <p data-testid="exchange-converted">
            {t("exchange.converted")}: {result.converted}
          </p>
          <p data-testid="exchange-excluded">
            {t("exchange.excluded")}: {result.excluded}
          </p>
          {result.as_of && (
            <p data-testid="exchange-asof">{t("exchange.asOf", { date: result.as_of })}</p>
          )}
          {!result.complete && (
            <p data-testid="exchange-incomplete" role="status">
              {t("exchange.incomplete")}
            </p>
          )}
        </div>
      )}
    </section>
  );
}
