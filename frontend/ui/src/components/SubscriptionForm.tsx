import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import { formatDate } from "../lib/format";
import type { components } from "../api/client";

type CategoryDto = components["schemas"]["CategoryDto"];

const UNITS = ["day", "week", "month", "year"] as const;

/**
 * Création d'un abonnement (REQ-SUB-002) : à la validation, l'abonnement est rattaché au compte et sa
 * prochaine échéance est affichée immédiatement. Erreurs de validation **par champ**. S'appuie sur le
 * client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SUB-002
 */
export function SubscriptionForm() {
  const { t, i18n } = useTranslation();
  const [name, setName] = useState("");
  const [amount, setAmount] = useState("9.99");
  const [currency, setCurrency] = useState("EUR");
  const [unit, setUnit] = useState<(typeof UNITS)[number]>("month");
  const [interval, setInterval] = useState("1");
  const [firstPayment, setFirstPayment] = useState("2030-01-31");
  const [endDate, setEndDate] = useState("");
  const [trialEnd, setTrialEnd] = useState("");
  const [category, setCategory] = useState("");
  const [categories, setCategories] = useState<CategoryDto[]>([]);
  const [nextPayment, setNextPayment] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Catégories du compte, source du sélecteur (REQ-CAT-001) : un abonnement peut être rattaché à une
  // catégorie. Chargées une fois au montage ; « (aucune) » laisse l'abonnement sans catégorie.
  useEffect(() => {
    void (async () => {
      const { data } = await api.GET("/categories");
      if (data) {
        setCategories(data);
      }
    })();
  }, []);

  async function submit() {
    const { data, error: err } = await api.POST("/subscriptions", {
      body: {
        name,
        amount,
        currency,
        cycle: { unit, interval: Number(interval) },
        first_payment: firstPayment,
        // Date de fin optionnelle (annulation programmée, REQ-SUB-009).
        ...(endDate.trim() !== "" ? { end_date: endDate.trim() } : {}),
        // Fin de période d'essai gratuit optionnelle (REQ-SUB-010).
        ...(trialEnd.trim() !== "" ? { trial_end: trialEnd.trim() } : {}),
        // Catégorie optionnelle (REQ-CAT-001) : omise si aucune n'est sélectionnée.
        ...(category !== "" ? { category } : {}),
      },
    });
    if (data) {
      setNextPayment(data.next_payment ?? null);
      setError(null);
    } else {
      // Erreur par champ : le `detail` RFC 9457 identifie le champ fautif.
      setError(err?.detail ?? t("subscription.error"));
      setNextPayment(null);
    }
  }

  return (
    <section data-testid="subscription-form" aria-label={t("subscription.title")}>
      <h2>{t("subscription.title")}</h2>

      <label>
        {t("subscription.name")}
        <input data-testid="sub-name" value={name} onChange={(e) => setName(e.target.value)} />
      </label>
      <label>
        {t("subscription.amount")}
        <input data-testid="sub-amount" value={amount} onChange={(e) => setAmount(e.target.value)} />
      </label>
      <label>
        {t("subscription.currency")}
        <input data-testid="sub-currency" value={currency} onChange={(e) => setCurrency(e.target.value)} />
      </label>
      <label>
        {t("subscription.unit")}
        <select
          data-testid="sub-cycle-unit"
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
        {t("subscription.interval")}
        <input data-testid="sub-cycle-interval" value={interval} onChange={(e) => setInterval(e.target.value)} />
      </label>
      <label>
        {t("subscription.firstPayment")}
        <input data-testid="sub-first-payment" value={firstPayment} onChange={(e) => setFirstPayment(e.target.value)} />
      </label>
      <label>
        {t("subscription.endDate")}
        <input data-testid="sub-end-date" value={endDate} onChange={(e) => setEndDate(e.target.value)} />
      </label>
      <label>
        {t("subscription.trialEnd")}
        <input data-testid="sub-trial-end" value={trialEnd} onChange={(e) => setTrialEnd(e.target.value)} />
      </label>
      <label>
        {t("subscription.category")}
        <select
          data-testid="sub-category"
          value={category}
          onChange={(e) => setCategory(e.target.value)}
        >
          <option value="">{t("subscription.noCategory")}</option>
          {categories.map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>

      <button type="button" data-testid="sub-submit" onClick={() => void submit()}>
        {t("subscription.create")}
      </button>

      {error && (
        <p data-testid="sub-error" role="alert">
          {error}
        </p>
      )}
      {nextPayment && (
        <p data-testid="sub-success" role="status">
          {t("subscription.created")}: {formatDate(nextPayment, i18n.language)}
        </p>
      )}
    </section>
  );
}
