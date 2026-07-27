import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { ConvertedTotalCard } from "./ConvertedTotalCard";

/** @implements REQ-CUR-004 */

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <ConvertedTotalCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ConvertedTotalCard", () => {
  it("affiche le total, la fraîcheur (date des taux) et pas de bandeau d'incomplétude quand complet", async () => {
    const post = vi.spyOn(api, "POST").mockResolvedValue(
      ok({
        total: "16.00",
        currency: "USD",
        converted: 2,
        excluded: 0,
        complete: true,
        as_of: "2026-07-20",
      }),
    );
    const user = userEvent.setup();
    renderCard();

    await user.clear(screen.getByTestId("exchange-target"));
    await user.type(screen.getByTestId("exchange-target"), "USD");
    await user.type(screen.getByTestId("exchange-amount-0"), "10");
    await user.type(screen.getByTestId("exchange-currency-0"), "EUR");
    await user.click(screen.getByTestId("exchange-submit"));

    expect(await screen.findByTestId("exchange-total")).toHaveTextContent("16.00 USD");
    // Fraîcheur affichée (mode dégradé : on indique la date du taux utilisé).
    expect(screen.getByTestId("exchange-asof")).toHaveTextContent(
      i18n.t("exchange.asOf", { date: "2026-07-20" }),
    );
    // Agrégat complet : aucun bandeau d'incomplétude.
    expect(screen.queryByTestId("exchange-incomplete")).toBeNull();

    // Montants transmis en CHAÎNE dans la devise cible (jamais un nombre JSON).
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/exchange/aggregate", {
        body: { target: "USD", amounts: [{ amount: "10", currency: "EUR" }] },
      }),
    );
  });

  it("signale explicitement un agrégat incomplet, jamais un zéro silencieux", async () => {
    vi.spyOn(api, "POST").mockResolvedValue(
      ok({
        total: "20",
        currency: "USD",
        converted: 1,
        excluded: 1,
        complete: false,
        as_of: null,
      }),
    );
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("exchange-submit"));

    // Le bandeau d'incomplétude est présent ; le total (part convertible) reste affiché.
    expect(await screen.findByTestId("exchange-incomplete")).toHaveTextContent(
      i18n.t("exchange.incomplete"),
    );
    expect(screen.getByTestId("exchange-excluded")).toHaveTextContent("1");
    expect(screen.getByTestId("exchange-total")).toHaveTextContent("20 USD");
    // Pas de date de fraîcheur quand aucun taux daté n'a servi.
    expect(screen.queryByTestId("exchange-asof")).toBeNull();
  });

  it("permet d'ajouter une ligne de montant", async () => {
    const user = userEvent.setup();
    renderCard();
    expect(screen.getAllByTestId("exchange-amount-row")).toHaveLength(1);
    await user.click(screen.getByTestId("exchange-add"));
    expect(screen.getAllByTestId("exchange-amount-row")).toHaveLength(2);
  });

  it("signale une erreur quand la requête échoue (ex. 422)", async () => {
    vi.spyOn(api, "POST").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("exchange-submit"));
    expect(await screen.findByTestId("exchange-error")).toHaveTextContent(
      i18n.t("exchange.error"),
    );
  });
});
