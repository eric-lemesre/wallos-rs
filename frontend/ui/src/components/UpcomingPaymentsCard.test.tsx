import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { UpcomingPaymentsCard } from "./UpcomingPaymentsCard";

/** @implements REQ-STA-005 */

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <UpcomingPaymentsCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("UpcomingPaymentsCard", () => {
  it("liste chaque occurrence de la fenêtre (plusieurs pour un même abonnement)", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValue(
      ok({
        from: "2025-01-01",
        to: "2025-03-31",
        payments: [
          { date: "2025-01-15", subscription_id: "s1", name: "Netflix", amount: "9.99", currency: "EUR" },
          { date: "2025-02-15", subscription_id: "s1", name: "Netflix", amount: "9.99", currency: "EUR" },
          { date: "2025-03-15", subscription_id: "s1", name: "Netflix", amount: "9.99", currency: "EUR" },
        ],
      }),
    );
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("upcoming-load"));

    const rows = await screen.findAllByTestId("upcoming-row");
    expect(rows).toHaveLength(3);
    expect(rows[0]).toHaveTextContent("Jan 15, 2025");
    expect(rows[0]).toHaveTextContent("Netflix");
    expect(rows[0]).toHaveTextContent("€9.99");
    // La fenêtre (jours) est transmise en paramètre de requête.
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/schedule/upcoming", {
        params: { query: { days: 30 } },
      }),
    );
  });

  it("affiche un état vide quand aucune échéance", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ from: "2025-01-01", to: "2025-01-31", payments: [] }));
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("upcoming-load"));
    expect(await screen.findByTestId("upcoming-empty")).toHaveTextContent(i18n.t("upcoming.empty"));
  });

  it("signale une erreur (ex. 422)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("upcoming-load"));
    expect(await screen.findByTestId("upcoming-error")).toHaveTextContent(i18n.t("upcoming.error"));
  });
});
