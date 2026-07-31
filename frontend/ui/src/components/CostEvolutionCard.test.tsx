import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { CostEvolutionCard } from "./CostEvolutionCard";

/** @implements REQ-STA-006 */

function response(points: { month: string; total: string }[]) {
  return {
    data: {
      currency: "EUR",
      from: points[0]?.month ?? "",
      to: points[points.length - 1]?.month ?? "",
      points,
    },
    response: new Response(null, { status: 200 }),
  } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <CostEvolutionCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("CostEvolutionCard", () => {
  it("charge et affiche la série mensuelle avec la devise", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      response([
        { month: "2026-05", total: "10.00" },
        { month: "2026-06", total: "15.00" },
      ]),
    );
    renderCard();

    const points = await screen.findAllByTestId("evolution-point");
    expect(points).toHaveLength(2);
    expect(screen.getAllByTestId("evolution-month")[0]).toHaveTextContent("2026-05");
    expect(screen.getAllByTestId("evolution-total")[1]).toHaveTextContent("15.00 EUR");
  });

  it("interroge l'endpoint d'évolution du coût", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValue(response([{ month: "2026-06", total: "0.00" }]));
    renderCard();
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/statistics/cost-evolution", { params: { query: {} } }),
    );
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderCard();
    expect(await screen.findByTestId("evolution-error")).toBeInTheDocument();
  });
});
