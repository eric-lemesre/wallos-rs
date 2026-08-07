import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import { formatAmount, formatDate } from "../lib/format";
import type { components } from "../api/client";
import { SubscriptionLogo } from "./SubscriptionLogo";

type SubscriptionListResponse = components["schemas"]["SubscriptionListResponse"];
type SubscriptionDto = components["schemas"]["SubscriptionDto"];
type CreateSubscriptionRequest = components["schemas"]["CreateSubscriptionRequest"];

type StateFilter = "all" | "active" | "inactive";
type SortBy = "name" | "amount" | "next_due";

const UNITS = ["day", "week", "month", "year"] as const;
const SORTS = ["name", "amount", "next_due"] as const;

/**
 * Vue par défaut de l'application (REQ-SUB-006) et **modification** en place (REQ-SUB-004) : liste des
 * abonnements du foyer, filtrable par catégorie et par état (filtres conjonctifs), avec un total qui
 * reflète le filtre. Chaque ligne est éditable : au changement de cycle/montant, le serveur **recalcule
 * la prochaine échéance** ré-ancrée sur la date de premier paiement. Chaque ligne est aussi
 * **supprimable** (REQ-SUB-005) : la suppression est traçable (pierre tombale, REQ-SYN-002) et retire
 * l'abonnement de la vue. Isolée par le serveur (§9). S'appuie exclusivement sur le client généré ;
 * aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SUB-006
 * @implements REQ-SUB-004
 * @implements REQ-SUB-005
 * @implements REQ-STA-007
 */
export function SubscriptionsList() {
  const { t, i18n } = useTranslation();
  const [category, setCategory] = useState("");
  const [payer, setPayer] = useState("");
  const [state, setState] = useState<StateFilter>("all");
  const [search, setSearch] = useState("");
  const [sort, setSort] = useState<SortBy>("name");
  const [data, setData] = useState<SubscriptionListResponse | null>(null);
  const [failed, setFailed] = useState(false);
  const [saveFailed, setSaveFailed] = useState(false);
  // Le total **reflète le filtre appliqué** (REQ-STA-007) : on mémorise si le dernier chargement
  // portait un filtre actif, pour l'**indiquer explicitement** à côté du total.
  const [filtered, setFiltered] = useState(false);

  // Chargement paramétré (jamais dans les deps d'un effet réactif : on ne recharge qu'au montage et
  // sur « Appliquer », pas à chaque frappe de filtre). Filtres conjonctifs : catégorie ET payeur ET état.
  const load = useCallback(
    async (cat: string, payerId: string, st: StateFilter, searchTerm: string, sortBy: SortBy) => {
    const query: {
      category?: string;
      payer?: string;
      active?: boolean;
      search?: string;
      sort?: string;
    } = {};
    if (cat.trim() !== "") {
      query.category = cat.trim();
    }
    if (payerId.trim() !== "") {
      query.payer = payerId.trim();
    }
    if (st !== "all") {
      query.active = st === "active";
    }
    // Recherche insensible casse+diacritiques sur nom ET notes ; tri (nom/montant/échéance) — REQ-SUB-007.
    if (searchTerm.trim() !== "") {
      query.search = searchTerm.trim();
    }
    if (sortBy !== "name") {
      query.sort = sortBy;
    }
    const { data: body, response } = await api.GET("/subscriptions", {
      params: { query },
    });
    if (response.ok && body) {
      setData(body);
      setFailed(false);
      // Un filtre est actif dès qu'un critère (catégorie, payeur, état ≠ « tous » ou recherche) s'applique.
      setFiltered(
        cat.trim() !== "" || payerId.trim() !== "" || st !== "all" || searchTerm.trim() !== "",
      );
    } else {
      setFailed(true);
    }
    },
    [],
  );

  useEffect(() => {
    void load("", "", "all", "", "name");
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
      await load(category, payer, state, search, sort);
      return true;
    },
    [load, category, payer, state, search, sort],
  );

  // Suppression traçable (REQ-SUB-005) : DELETE puis rechargement ; l'abonnement disparaît de la vue.
  // Tout échec (statut HTTP ou exception réseau) est signalé, jamais un rechargement silencieux.
  const remove = useCallback(
    async (sub: SubscriptionDto) => {
      try {
        const { response } = await api.DELETE("/subscriptions/{id}", {
          params: { path: { id: sub.id } },
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
      await load(category, payer, state, search, sort);
      return true;
    },
    [load, category, payer, state, search, sort],
  );

  const subscriptions = data?.subscriptions ?? [];

  return (
    <section data-testid="subscriptions-list" aria-label={t("subscriptions.title")}>
      <h2>{t("subscriptions.title")}</h2>

      <div>
        <input
          data-testid="subscriptions-search"
          aria-label={t("subscriptions.search")}
          placeholder={t("subscriptions.searchPlaceholder")}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <select
          data-testid="subscriptions-sort"
          aria-label={t("subscriptions.sort")}
          value={sort}
          onChange={(e) => setSort(e.target.value as SortBy)}
        >
          {SORTS.map((s) => (
            <option key={s} value={s}>
              {t(`subscriptions.sortBy.${s}`)}
            </option>
          ))}
        </select>
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
          onClick={() => void load(category, payer, state, search, sort)}
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
            <SubscriptionRow key={sub.id} sub={sub} onSave={save} onDelete={remove} />
          ))}
        </ul>
      )}

      {data && (
        <div data-testid="subscriptions-total">
          {t("subscriptions.total")}: {formatAmount(data.total.total, data.total.currency, i18n.language)}
          {filtered && (
            <span data-testid="subscriptions-total-filtered">
              {" "}
              {t("subscriptions.totalFiltered")}
            </span>
          )}
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
  onDelete,
}: {
  sub: SubscriptionDto;
  onSave: (
    sub: SubscriptionDto,
    patch: { amount: string; unit: string; interval: number; active: boolean },
  ) => Promise<boolean>;
  onDelete: (sub: SubscriptionDto) => Promise<boolean>;
}) {
  const { t, i18n } = useTranslation();
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
        {formatAmount(sub.amount, sub.currency, i18n.language)}
      </span>
      <span data-testid="subscription-monthly">
        {t("subscriptions.monthlyCost")}: {formatAmount(sub.monthly_cost ?? "", sub.currency, i18n.language)}
      </span>
      <span data-testid="subscription-yearly">
        {t("subscriptions.yearlyCost")}: {formatAmount(sub.yearly_cost ?? "", sub.currency, i18n.language)}
      </span>
      {sub.in_trial && (
        <span data-testid="subscription-trial">{t("subscriptions.trial")}</span>
      )}
      {sub.ended ? (
        <span data-testid="subscription-ended">{t("subscriptions.ended")}</span>
      ) : (
        sub.next_payment && (
          <span data-testid="subscription-next">
            {t("subscriptions.nextPayment")}: {formatDate(sub.next_payment, i18n.language)}
          </span>
        )
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
        <span>
          <button
            type="button"
            data-testid="subscription-edit"
            onClick={() => setEditing(true)}
          >
            {t("subscriptions.edit")}
          </button>
          <button
            type="button"
            data-testid={`subscription-delete-${sub.id}`}
            onClick={() => void onDelete(sub)}
          >
            {t("subscriptions.delete")}
          </button>
        </span>
      )}
    </li>
  );
}
