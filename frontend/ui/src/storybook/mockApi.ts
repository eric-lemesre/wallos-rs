import { http, HttpResponse } from "msw";
import type { components } from "@wallos/api-client";

/**
 * Simulation de l'API pour l'atelier (ADR 0058).
 *
 * On n'intercepte que la **frontière réseau** : le client généré, l'i18n, les hooks et toute la
 * logique des écrans s'exécutent inchangés. Ce que montre l'atelier est donc ce que verra
 * l'utilisateur — à la différence de props factices, qui obligeraient à réécrire les écrans pour
 * les afficher et feraient regarder autre chose que ce qui tourne.
 *
 * Les fixtures sont **typées contre le contrat généré** : une évolution de l'API qui les
 * invaliderait casse le `typecheck`, au lieu de laisser l'atelier montrer un état qui n'existe plus.
 */

/**
 * Même préfixe que le client (`api/client.ts`). L'astérisque de tête fait correspondre **n'importe
 * quelle origine** : l'atelier sert des URL relatives à `localhost:6006`, tandis que le garde-fou
 * (`mockApi.test.tsx`) doit fournir une origine absolue, faute de pouvoir en résoudre une hors
 * navigateur. Un seul jeu de gestionnaires couvre ainsi les deux.
 */
const BASE = "*/api/v1";

export type ReminderDto = components["schemas"]["ReminderDto"];
export type ReminderSettingResponse = components["schemas"]["ReminderSettingResponse"];
export type RemindersResponse = components["schemas"]["RemindersResponse"];

/** Délai de rappel du compte. */
export function handlerReminderSetting(leadDays: number) {
  const corps: ReminderSettingResponse = { lead_days: leadDays };
  return http.get(`${BASE}/settings/reminder`, () => HttpResponse.json(corps));
}

/** Rappels dus. Une liste vide est un état à part entière, pas une absence de gestionnaire. */
export function handlerReminders(reminders: ReminderDto[], asOf = "2026-08-16") {
  const corps: RemindersResponse = { as_of: asOf, reminders };
  return http.get(`${BASE}/reminders`, () => HttpResponse.json(corps));
}

/**
 * Panne du service. Le corps suit RFC 9457 (`application/problem+json`, REQ-SEC-002) : simuler une
 * erreur avec une forme que l'API n'émet pas donnerait un état d'erreur trompeur.
 */
export function handlerEnPanne(chemin: string, statut = 503) {
  return http.get(`${BASE}${chemin}`, () =>
    HttpResponse.json(
      { type: "about:blank", title: "Service Unavailable", status: statut },
      { status: statut, headers: { "content-type": "application/problem+json" } },
    ),
  );
}

/**
 * Quelques rappels plausibles — jamais de donnée réelle, jamais de secret.
 *
 * Les deux `kind` émis par le serveur sont représentés : une échéance de paiement et une fin
 * d'essai. Ne montrer que le premier laisserait le second sans aucune vue dans l'atelier, alors
 * que c'est celui dont la formulation est la plus facile à manquer.
 */
export const RAPPELS: ReminderDto[] = [
  { subscription_id: "1", name: "Netflix", due_date: "2026-08-18", days_until: 2, kind: "payment" },
  { subscription_id: "2", name: "Spotify", due_date: "2026-08-21", days_until: 5, kind: "payment" },
  {
    subscription_id: "3",
    name: "Hébergement",
    due_date: "2026-08-23",
    days_until: 7,
    kind: "trial_ending",
  },
];
