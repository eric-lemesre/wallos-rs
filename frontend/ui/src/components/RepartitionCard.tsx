import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import { formatAmount } from "../lib/format";
import type { components } from "../api/client";

type RepartitionResponse = components["schemas"]["RepartitionResponse"];
type RepartitionEntry = components["schemas"]["RepartitionEntryDto"];

/**
 * Répartition des coûts mensuels par catégorie **et** par payeur (REQ-STA-004). Deux axes partageant
 * la même mécanique : la somme des parts d'un axe égale le total général affiché. Une part sans libellé
 * (`label` absent) correspond aux abonnements **sans catégorie / sans payeur** et est rendue par un
 * libellé localisé « (aucun) », **jamais omise** (critère d'acceptation #2). Les montants sont exprimés
 * dans la devise de référence du foyer. S'appuie exclusivement sur le client généré ; aucune chaîne
 * d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-STA-004
 */
export function RepartitionCard() {
  const { t, i18n } = useTranslation();
  const [result, setResult] = useState<RepartitionResponse | null>(null);
  const [failed, setFailed] = useState(false);

  async function load() {
    const { data, response } = await api.GET("/statistics/repartition", {
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

  // Un mini-histogramme accessible par axe : proportion du bucket le plus lourd de l'axe.
  function renderAxis(testid: string, heading: string, entries: RepartitionEntry[]) {
    const max = entries.reduce((m, e) => Math.max(m, Number(e.total)), 0);
    return (
      <div data-testid={`repartition-axis-${testid}`}>
        <h3>{heading}</h3>
        {entries.length === 0 ? (
          <p data-testid={`repartition-empty-${testid}`}>{t("repartition.empty")}</p>
        ) : (
          <ol data-testid={`repartition-list-${testid}`}>
            {entries.map((e, i) => {
              const label = e.label ?? t("repartition.none");
              const ratio = max > 0 ? Math.round((Number(e.total) / max) * 100) : 0;
              return (
                <li
                  key={`${e.label ?? "__none__"}-${i}`}
                  data-testid={`repartition-entry-${testid}`}
                >
                  <span data-testid="repartition-label">{label}</span>
                  <span data-testid="repartition-total">
                    {formatAmount(e.total, result?.currency ?? "", i18n.language)}
                  </span>
                  <span
                    data-testid="repartition-bar"
                    role="img"
                    aria-label={`${label}: ${formatAmount(e.total, result?.currency ?? "", i18n.language)}`}
                    style={{ display: "inline-block", width: `${ratio}%` }}
                  />
                </li>
              );
            })}
          </ol>
        )}
      </div>
    );
  }

  return (
    <section data-testid="repartition-card" aria-label={t("repartition.title")}>
      <h2>{t("repartition.title")}</h2>
      <button type="button" data-testid="repartition-reload" onClick={() => void load()}>
        {t("repartition.reload")}
      </button>

      {failed && (
        <p data-testid="repartition-error" role="alert">
          {t("repartition.error")}
        </p>
      )}

      {result && !result.complete && (
        <p data-testid="repartition-incomplete" role="status">
          {t("repartition.incomplete")}
        </p>
      )}

      {result && (
        <>
          <p data-testid="repartition-grand-total">
            {t("repartition.total")}: {formatAmount(result.total, result.currency, i18n.language)}
          </p>
          {renderAxis("category", t("repartition.byCategory"), result.by_category)}
          {renderAxis("payer", t("repartition.byPayer"), result.by_payer)}
        </>
      )}
    </section>
  );
}
