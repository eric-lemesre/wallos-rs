import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import { formatAmount, formatDate } from "../lib/format";
import type { components } from "../api/client";

type UpcomingPaymentsResponse = components["schemas"]["UpcomingPaymentsResponse"];

/**
 * Échéancier des prochains paiements (REQ-STA-005) : liste toutes les occurrences attendues sur une
 * fenêtre de N jours, un même abonnement pouvant apparaître plusieurs fois. S'appuie sur le client
 * généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-STA-005
 */
export function UpcomingPaymentsCard() {
  const { t, i18n } = useTranslation();
  const [days, setDays] = useState("30");
  const [result, setResult] = useState<UpcomingPaymentsResponse | null>(null);
  const [failed, setFailed] = useState(false);

  async function load() {
    const { data, response } = await api.GET("/schedule/upcoming", {
      params: { query: { days: Number(days) } },
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
    <section data-testid="upcoming-card" aria-label={t("upcoming.title")}>
      <h2>{t("upcoming.title")}</h2>

      <label>
        {t("upcoming.days")}
        <input
          data-testid="upcoming-days"
          value={days}
          onChange={(e) => setDays(e.target.value)}
        />
      </label>
      <button type="button" data-testid="upcoming-load" onClick={() => void load()}>
        {t("upcoming.load")}
      </button>

      {failed && (
        <p data-testid="upcoming-error" role="alert">
          {t("upcoming.error")}
        </p>
      )}
      {result &&
        (result.payments.length === 0 ? (
          <p data-testid="upcoming-empty">{t("upcoming.empty")}</p>
        ) : (
          <ul data-testid="upcoming-list">
            {result.payments.map((p, index) => (
              <li key={`${p.subscription_id}-${p.date}-${index}`} data-testid="upcoming-row">
                <span data-testid="upcoming-date">{formatDate(p.date, i18n.language)}</span>
                <span data-testid="upcoming-name">{p.name}</span>
                <span data-testid="upcoming-amount">
                  {formatAmount(p.amount, p.currency, i18n.language)}
                </span>
              </li>
            ))}
          </ul>
        ))}
    </section>
  );
}
