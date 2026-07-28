import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type SubscriptionListResponse = components["schemas"]["SubscriptionListResponse"];

type StateFilter = "all" | "active" | "inactive";

/**
 * Vue par défaut de l'application (REQ-SUB-006) : liste des abonnements du foyer, filtrable par
 * catégorie et par état. Les filtres sont **conjonctifs** et le **total affiché reflète le filtre**
 * (agrégat des abonnements actifs retournés, réutilisant la conversion multi-devises en mode dégradé,
 * REQ-CUR-004). Isolée par le serveur (§9). S'appuie exclusivement sur le client généré ; aucune
 * chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SUB-006
 */
export function SubscriptionsList() {
  const { t } = useTranslation();
  const [category, setCategory] = useState("");
  const [state, setState] = useState<StateFilter>("all");
  const [data, setData] = useState<SubscriptionListResponse | null>(null);
  const [failed, setFailed] = useState(false);

  // Chargement paramétré (jamais dans les deps d'un effet réactif : on ne recharge qu'au montage et
  // sur « Appliquer », pas à chaque frappe de filtre).
  const load = useCallback(async (cat: string, st: StateFilter) => {
    const query: { category?: string; active?: boolean } = {};
    if (cat.trim() !== "") {
      query.category = cat.trim();
    }
    if (st !== "all") {
      query.active = st === "active";
    }
    const { data: body, response } = await api.GET("/subscriptions", {
      params: { query },
    });
    if (response.ok && body) {
      setData(body);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void load("", "all");
  }, [load]);

  const subscriptions = data?.subscriptions ?? [];

  return (
    <section data-testid="subscriptions-list" aria-label={t("subscriptions.title")}>
      <h2>{t("subscriptions.title")}</h2>

      <div>
        <input
          data-testid="subscriptions-filter-category"
          aria-label={t("subscriptions.filterCategory")}
          value={category}
          onChange={(e) => setCategory(e.target.value)}
        />
        <select
          data-testid="subscriptions-filter-state"
          aria-label={t("subscriptions.filterState")}
          value={state}
          onChange={(e) => setState(e.target.value as StateFilter)}
        >
          <option value="all">{t("subscriptions.stateAll")}</option>
          <option value="active">{t("subscriptions.stateActive")}</option>
          <option value="inactive">{t("subscriptions.stateInactive")}</option>
        </select>
        <button
          type="button"
          data-testid="subscriptions-apply"
          onClick={() => void load(category, state)}
        >
          {t("subscriptions.apply")}
        </button>
      </div>

      {failed && (
        <p data-testid="subscriptions-error" role="alert">
          {t("subscriptions.error")}
        </p>
      )}

      {subscriptions.length === 0 ? (
        <p data-testid="subscriptions-empty">{t("subscriptions.empty")}</p>
      ) : (
        <ul>
          {subscriptions.map((sub) => (
            <li key={sub.id} data-testid="subscription-row">
              <span data-testid="subscription-name">{sub.name}</span>
              <span data-testid="subscription-amount">
                {sub.amount} {sub.currency}
              </span>
              {sub.next_payment && (
                <span data-testid="subscription-next">
                  {t("subscriptions.nextPayment")}: {sub.next_payment}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}

      {data && (
        <div data-testid="subscriptions-total">
          {t("subscriptions.total")}: {data.total.total} {data.total.currency}
          {!data.total.complete && (
            <span data-testid="subscriptions-total-incomplete" role="status">
              {t("subscriptions.incomplete")}
            </span>
          )}
        </div>
      )}
    </section>
  );
}
