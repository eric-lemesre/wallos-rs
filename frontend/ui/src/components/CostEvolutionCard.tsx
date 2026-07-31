import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type CostEvolutionResponse = components["schemas"]["CostEvolutionResponse"];

/**
 * Évolution du coût mensuel sur douze mois (REQ-STA-006) : une série où **chaque point reflète les
 * abonnements actifs à ce mois-là** (via leurs dates de début/fin), pas l'état courant projeté. Le
 * total mensuel est exprimé dans la devise de référence du foyer. S'appuie exclusivement sur le client
 * généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-STA-006
 */
export function CostEvolutionCard() {
  const { t } = useTranslation();
  const [result, setResult] = useState<CostEvolutionResponse | null>(null);
  const [failed, setFailed] = useState(false);

  async function load() {
    const { data, response } = await api.GET("/statistics/cost-evolution", {
      params: { query: {} },
    });
    if (response.ok && data) {
      setResult(data);
      setFailed(false);
    } else {
      setResult(null);
      setFailed(true);
    }
  }

  useEffect(() => {
    void load();
  }, []);

  // Échelle relative pour un mini-histogramme accessible : proportion du total le plus élevé de la série.
  const points = result?.points ?? [];
  const max = points.reduce((m, p) => Math.max(m, Number(p.total)), 0);

  return (
    <section data-testid="evolution-card" aria-label={t("evolution.title")}>
      <h2>{t("evolution.title")}</h2>
      <button type="button" data-testid="evolution-reload" onClick={() => void load()}>
        {t("evolution.reload")}
      </button>

      {failed && (
        <p data-testid="evolution-error" role="alert">
          {t("evolution.error")}
        </p>
      )}

      {result && !result.complete && (
        <p data-testid="evolution-incomplete" role="status">
          {t("evolution.incomplete")}
        </p>
      )}

      {result && (
        <ol data-testid="evolution-list">
          {points.map((p) => {
            const ratio = max > 0 ? Math.round((Number(p.total) / max) * 100) : 0;
            return (
              <li key={p.month} data-testid="evolution-point">
                <span data-testid="evolution-month">{p.month}</span>
                <span data-testid="evolution-total">
                  {p.total} {result.currency}
                </span>
                <span
                  data-testid="evolution-bar"
                  role="img"
                  aria-label={`${p.total} ${result.currency}`}
                  style={{ display: "inline-block", width: `${ratio}%` }}
                />
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}
