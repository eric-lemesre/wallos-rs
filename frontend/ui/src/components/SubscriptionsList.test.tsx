import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { SubscriptionsList } from "./SubscriptionsList";

/** @implements REQ-SUB-006 */

const CAT1 = "11111111-1111-1111-1111-111111111111";

function sub(id: string, name: string, amount: string) {
  return {
    id,
    name,
    amount,
    currency: "EUR",
    cycle: { unit: "month", interval: 1 },
    first_payment: "2030-01-15",
    active: true,
    next_payment: "2030-01-15",
  };
}

function response(subscriptions: unknown[], total: string) {
  return {
    data: {
      subscriptions,
      total: {
        total,
        currency: "EUR",
        converted: subscriptions.length,
        excluded: 0,
        complete: true,
      },
    },
    response: new Response(null, { status: 200 }),
  } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <SubscriptionsList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("SubscriptionsList", () => {
  it("liste les abonnements et affiche le total", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      response([sub("1", "Netflix", "9.99"), sub("2", "Spotify", "5.99")], "15.98"),
    );
    renderList();

    expect(await screen.findAllByTestId("subscription-name")).toHaveLength(2);
    expect(screen.getByTestId("subscriptions-total")).toHaveTextContent("15.98 EUR");
  });

  it("applique un filtre catégorie + état (query conjonctive)", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValue(response([sub("1", "Netflix", "9.99")], "9.99"));
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("subscription-name");
    await user.type(screen.getByTestId("subscriptions-filter-category"), CAT1);
    await user.selectOptions(screen.getByTestId("subscriptions-filter-state"), "active");
    await user.click(screen.getByTestId("subscriptions-apply"));

    await waitFor(() =>
      expect(get).toHaveBeenLastCalledWith("/subscriptions", {
        params: { query: { category: CAT1, active: true } },
      }),
    );
  });

  it("modifie un abonnement en place (PUT, échéance recalculée côté serveur)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(response([sub("1", "Netflix", "9.99")], "9.99"));
    const put = vi.spyOn(api, "PUT").mockResolvedValue({
      data: { ...sub("1", "Netflix", "19.99"), cycle: { unit: "year", interval: 1 } },
      response: new Response(null, { status: 200 }),
    } as never);
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("subscription-name");
    await user.click(screen.getByTestId("subscription-edit"));
    const amount = screen.getByTestId("subscription-amount-input");
    await user.clear(amount);
    await user.type(amount, "19.99");
    await user.selectOptions(screen.getByTestId("subscription-unit"), "year");
    await user.click(screen.getByTestId("subscription-save"));

    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/subscriptions/{id}", {
        params: { path: { id: "1" } },
        body: expect.objectContaining({
          name: "Netflix",
          amount: "19.99",
          currency: "EUR",
          cycle: { unit: "year", interval: 1 },
        }),
      }),
    );
  });

  it("désactive un abonnement (PUT active=false, REQ-SUB-008)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(response([sub("1", "Netflix", "9.99")], "9.99"));
    const put = vi.spyOn(api, "PUT").mockResolvedValue({
      data: { ...sub("1", "Netflix", "9.99"), active: false },
      response: new Response(null, { status: 200 }),
    } as never);
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("subscription-name");
    await user.click(screen.getByTestId("subscription-edit"));
    await user.click(screen.getByTestId("subscription-active")); // décoche (actif -> inactif)
    await user.click(screen.getByTestId("subscription-save"));

    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/subscriptions/{id}", {
        params: { path: { id: "1" } },
        body: expect.objectContaining({ active: false }),
      }),
    );
  });

  it("filtre par payeur (query conjonctive)", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValue(response([sub("1", "Netflix", "9.99")], "9.99"));
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("subscription-name");
    await user.type(
      screen.getByTestId("subscriptions-filter-payer"),
      "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
    );
    await user.click(screen.getByTestId("subscriptions-apply"));
    await waitFor(() =>
      expect(get).toHaveBeenLastCalledWith("/subscriptions", {
        params: { query: { payer: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" } },
      }),
    );
  });

  it("signale un échec d'enregistrement (PUT en erreur, revue SUB-004 #2)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(response([sub("1", "Netflix", "9.99")], "9.99"));
    vi.spyOn(api, "PUT").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("subscription-name");
    await user.click(screen.getByTestId("subscription-edit"));
    await user.click(screen.getByTestId("subscription-save"));
    // L'erreur est visible et le mode édition reste ouvert (champ montant toujours présent).
    expect(await screen.findByTestId("subscriptions-save-error")).toBeInTheDocument();
    expect(screen.getByTestId("subscription-amount-input")).toBeInTheDocument();
  });

  it("affiche un message quand la liste est vide", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(response([], "0.00"));
    renderList();
    expect(await screen.findByTestId("subscriptions-empty")).toBeInTheDocument();
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderList();
    expect(await screen.findByTestId("subscriptions-error")).toBeInTheDocument();
  });
});
