import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type NextDueResponse = components["schemas"]["NextDueResponse"];

const UNITS = ["day", "week", "month", "year"] as const;

/**
 * Calcul de la prochaine échéance (REQ-SUB-012) : ancrage sur le jour d'origine + clamp fin de mois
 * (ADR 0022). S'appuie sur le client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SUB-012
 */
export function NextDueCard() {
  const { t } = useTranslation();
  const [anchor, setAnchor] = useState("2025-01-31");
  const [unit, setUnit] = useState<(typeof UNITS)[number]>("month");
  const [interval, setInterval] = useState("1");
  const [after, setAfter] = useState("2025-01-31");
  const [result, setResult] = useState<NextDueResponse | null>(null);
  const [failed, setFailed] = useState(false);

  async function compute() {
    const { data, response } = await api.POST("/schedule/next-due", {
      body: {
        first_payment: anchor,
        cycle: { unit, interval: Number(interval) },
        after,
      },
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
    <section data-testid="nextdue-card" aria-label={t("nextDue.title")}>
      <h2>{t("nextDue.title")}</h2>

      <label>
        {t("nextDue.firstPayment")}
        <input data-testid="nextdue-anchor" value={anchor} onChange={(e) => setAnchor(e.target.value)} />
      </label>
      <label>
        {t("nextDue.unit")}
        <select
          data-testid="nextdue-unit"
          value={unit}
          onChange={(e) => setUnit(e.target.value as (typeof UNITS)[number])}
        >
          {UNITS.map((u) => (
            <option key={u} value={u}>
              {t(`nextDue.units.${u}`)}
            </option>
          ))}
        </select>
      </label>
      <label>
        {t("nextDue.interval")}
        <input data-testid="nextdue-interval" value={interval} onChange={(e) => setInterval(e.target.value)} />
      </label>
      <label>
        {t("nextDue.after")}
        <input data-testid="nextdue-after" value={after} onChange={(e) => setAfter(e.target.value)} />
      </label>

      <button type="button" data-testid="nextdue-compute" onClick={() => void compute()}>
        {t("nextDue.compute")}
      </button>

      {failed && (
        <p data-testid="nextdue-error" role="alert">
          {t("nextDue.error")}
        </p>
      )}
      {result && (
        <p data-testid="nextdue-result">
          {t("nextDue.result")}: {result.next_payment}
        </p>
      )}
    </section>
  );
}
