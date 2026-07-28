import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";
import { SubscriptionLogo } from "./SubscriptionLogo";

type SubscriptionListResponse = components["schemas"]["SubscriptionListResponse"];
type SubscriptionDto = components["schemas"]["SubscriptionDto"];
type CreateSubscriptionRequest = components["schemas"]["CreateSubscriptionRequest"];

type StateFilter = "all" | "active" | "inactive";

const UNITS = ["day", "week", "month", "year"] as const;

/**
 * Vue par défaut de l'application (REQ-SUB-006) et **modification** en place (REQ-SUB-004) : liste des
 * abonnements du foyer, filtrable par catégorie et par état (filtres conjonctifs), avec un total qui
 * reflète le filtre. Chaque ligne est éditable : au changement de cycle/montant, le serveur **recalcule
 * la prochaine échéance** ré-ancrée sur la date de premier paiement. Isolée par le serveur (§9).
 * S'appuie exclusivement sur le client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SUB-006
 * @implements REQ-SUB-004
 */
export function SubscriptionsList() {
  const { t } = useTranslation();
  const [category, setCategory] = useState("");
  const [payer, setPayer] = useState("");
  const [state, setState] = useState<StateFilter>("all");
  const [data, setData] = useState<SubscriptionListResponse | null>(null);
  const [failed, setFailed] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);

  // Chargement paramétré (jamais dans les deps d'un effet réactif : on ne recharge qu'au montage et
  // sur « Appliquer », pas à chaque frappe de filtre). Filtres conjonctifs : catégorie ET payeur ET état.
  const load = useCallback(async (cat: string, payerId: string, st: StateFilter) => {
    const query: { category?: string; payer?: string; active?: boolean } = {};
    if (cat.trim() !== "") {
      query.category = cat.trim();
    }
    if (payerId.trim() !== "") {
      query.payer = payerId.trim();
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
    void load("", "", "all");
  }, [load]);

  const save = useCallback(
    async (
      sub: SubscriptionDto,
      patch: { amount: string; unit: string; interval: number; active: boolean },
    ) => {
      const body: CreateSubscriptionRequest = {
        name: sub.name,
        amount: patch.amount,
        currency: sub.currency,
        cycle: { unit: patch.unit, interval: patch.interval },
        first_payment: sub.first_payment,
        category: sub.category,
        payment_method: sub.payment_method,
        payer: sub.payer,
        logo: sub.logo,
        url: sub.url,
        notes: sub.notes,
        active: patch.active,
      };
      // Revue SUB-004 #2 / SUB-008 : tout échec du PUT — statut HTTP d'erreur **ou** exception réseau
      // (fetch rejeté) — est signalé (jamais un rechargement silencieux, jamais une exception non gérée).
      try {
        const { response } = await api.PUT("/subscriptions/{id}", {
          params: { path: { id: sub.id } },
          body,
        });
        if (!response.ok) {
          setSaveFailed(true);
          return false;
        }
      } catch {
        setSaveFailed(true);
        return false;
      }
      setSaveFailed(false);
      await load(category, payer, state);
      return true;
    },
    [load, category, payer, state],
  );

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
        <input
          data-testid="subscriptions-filter-payer"
          aria-label={t("subscriptions.filterPayer")}
          value={payer}
          onChange={(e) => setPayer(e.target.value)}
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
          onClick={() => void load(category, payer, state)}
        >
          {t("subscriptions.apply")}
        </button>
      </div>

      {failed && (
        <p data-testid="subscriptions-error" role="alert">
          {t("subscriptions.error")}
        </p>
      )}

      {saveFailed && (
        <p data-testid="subscriptions-save-error" role="alert">
          {t("subscriptions.saveError")}
        </p>
      )}

      {subscriptions.length === 0 ? (
        <p data-testid="subscriptions-empty">{t("subscriptions.empty")}</p>
      ) : (
        <ul>
          {subscriptions.map((sub) => (
            <SubscriptionRow key={sub.id} sub={sub} onSave={save} />
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

function SubscriptionRow({
  sub,
  onSave,
}: {
  sub: SubscriptionDto;
  onSave: (
    sub: SubscriptionDto,
    patch: { amount: string; unit: string; interval: number; active: boolean },
  ) => Promise<boolean>;
}) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState(false);
  const [amount, setAmount] = useState(sub.amount);
  const [unit, setUnit] = useState(sub.cycle.unit);
  const [interval, setInterval] = useState(String(sub.cycle.interval));
  const [active, setActive] = useState(sub.active ?? true);

  async function submit() {
    const ok = await onSave(sub, {
      amount,
      unit,
      interval: Number.parseInt(interval, 10) || sub.cycle.interval,
      active,
    });
    // On ne quitte le mode édition qu'en cas de succès (l'erreur reste visible pour correction).
    if (ok) {
      setEditing(false);
    }
  }

  return (
    <li data-testid="subscription-row">
      <SubscriptionLogo name={sub.name} logo={sub.logo} />
      <span data-testid="subscription-name">{sub.name}</span>
      <span data-testid="subscription-amount">
        {sub.amount} {sub.currency}
      </span>
      {sub.next_payment && (
        <span data-testid="subscription-next">
          {t("subscriptions.nextPayment")}: {sub.next_payment}
        </span>
      )}

      {editing ? (
        <span>
          <input
            data-testid="subscription-amount-input"
            aria-label={t("subscriptions.editAmount")}
            value={amount}
            onChange={(e) => setAmount(e.target.value)}
          />
          <select
            data-testid="subscription-unit"
            aria-label={t("subscriptions.editUnit")}
            value={unit}
            onChange={(e) => setUnit(e.target.value)}
          >
            {UNITS.map((u) => (
              <option key={u} value={u}>
                {t(`nextDue.units.${u}`)}
              </option>
            ))}
          </select>
          <input
            data-testid="subscription-interval"
            aria-label={t("subscriptions.editInterval")}
            value={interval}
            onChange={(e) => setInterval(e.target.value)}
          />
          <label>
            {t("subscriptions.editActive")}
            <input
              type="checkbox"
              data-testid="subscription-active"
              checked={active}
              onChange={(e) => setActive(e.target.checked)}
            />
          </label>
          <button type="button" data-testid="subscription-save" onClick={() => void submit()}>
            {t("subscriptions.save")}
          </button>
        </span>
      ) : (
        <button
          type="button"
          data-testid="subscription-edit"
          onClick={() => setEditing(true)}
        >
          {t("subscriptions.edit")}
        </button>
      )}
    </li>
  );
}
