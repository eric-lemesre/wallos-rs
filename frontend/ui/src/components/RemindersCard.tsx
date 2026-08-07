import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import { formatDate } from "../lib/format";
import type { components } from "../api/client";

type Reminder = components["schemas"]["ReminderDto"];

/**
 * Rappels avant échéance (REQ-NOT-001) : règle le **délai de rappel** du compte (jours avant l'échéance)
 * et liste les **rappels dus aujourd'hui** — le pendant lisible du message unique émis par le cron. Les
 * rappels sont regroupés par compte (un seul bloc). Isolée par le serveur (§9). S'appuie sur le client
 * généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * Cette vue in-app **est** la surface de consultation des rappels : elle satisfait le volet applicable
 * de REQ-NOT-008 (« l'information reste consultable dans l'interface »), la notification **native** étant
 * hors périmètre (OQ-009, ADR 0045 — pas de coquille native pour la parité, comme SEC-006/AUT-005).
 *
 * @implements REQ-NOT-001
 * @implements REQ-NOT-008
 */
export function RemindersCard() {
  const { t, i18n } = useTranslation();
  const [leadDays, setLeadDays] = useState<number | null>(null);
  const [draft, setDraft] = useState("");
  const [reminders, setReminders] = useState<Reminder[]>([]);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(async () => {
    const setting = await api.GET("/settings/reminder");
    const list = await api.GET("/reminders");
    if (setting.response.ok && setting.data && list.response.ok && list.data) {
      setLeadDays(setting.data.lead_days);
      setDraft(String(setting.data.lead_days));
      setReminders(list.data.reminders);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function save() {
    const lead = Number.parseInt(draft, 10);
    const { data, response } = await api.PUT("/settings/reminder", {
      body: { lead_days: Number.isNaN(lead) ? 1 : lead },
    });
    if (response.ok && data) {
      setLeadDays(data.lead_days);
      setFailed(false);
      await refresh();
    } else {
      setFailed(true);
    }
  }

  return (
    <section data-testid="reminders-card" aria-label={t("reminders.title")}>
      <h2>{t("reminders.title")}</h2>

      {failed && (
        <p data-testid="reminders-error" role="alert">
          {t("reminders.loadError")}
        </p>
      )}

      <label>
        {t("reminders.leadLabel")}
        <input
          data-testid="reminders-lead-input"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          inputMode="numeric"
        />
      </label>
      <button type="button" data-testid="reminders-save" onClick={() => void save()}>
        {t("reminders.save")}
      </button>
      {leadDays !== null && (
        <p data-testid="reminders-lead-current">
          {t("reminders.leadCurrent", { count: leadDays })}
        </p>
      )}

      {reminders.length === 0 ? (
        <p data-testid="reminders-empty">{t("reminders.empty")}</p>
      ) : (
        <ul data-testid="reminders-list">
          {reminders.map((r) => (
            <li key={r.subscription_id} data-testid="reminder-item">
              <span data-testid="reminder-name">{r.name}</span>
              <span data-testid="reminder-when">
                {t("reminders.dueIn", { count: r.days_until })} — {formatDate(r.due_date, i18n.language)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
