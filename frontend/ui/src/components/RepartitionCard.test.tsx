import { render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { RepartitionCard } from "./RepartitionCard";

/** @implements REQ-STA-004 */

type Entry = { label?: string | null; total: string; count: number };

function response(
  total: string,
  byCategory: Entry[],
  byPayer: Entry[],
  complete = true,
) {
  return {
    data: {
      currency: "EUR",
      total,
      complete,
      by_category: byCategory,
      by_payer: byPayer,
    },
    response: new Response(null, { status: 200 }),
  } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <RepartitionCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("RepartitionCard", () => {
  it("affiche les deux axes avec la devise et le total général", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      response(
        "25.00",
        [
          { label: "Streaming", total: "20.00", count: 2 },
          { label: null, total: "5.00", count: 1 },
        ],
        [
          { label: "Alex", total: "20.00", count: 2 },
          { label: null, total: "5.00", count: 1 },
        ],
      ),
    );
    renderCard();

    expect(await screen.findByTestId("repartition-grand-total")).toHaveTextContent("€25.00");
    const catEntries = screen.getAllByTestId("repartition-entry-category");
    expect(catEntries).toHaveLength(2);
    expect(catEntries[0]).toHaveTextContent("Streaming");
    expect(catEntries[0]).toHaveTextContent("€20.00");
    const payerEntries = screen.getAllByTestId("repartition-entry-payer");
    expect(payerEntries).toHaveLength(2);
    expect(payerEntries[0]).toHaveTextContent("Alex");
  });

  it("rend l'entrée sans axe (label null) par le libellé localisé « (aucun) »", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      response(
        "12.00",
        [{ label: null, total: "12.00", count: 1 }],
        [{ label: null, total: "12.00", count: 1 }],
      ),
    );
    renderCard();

    const labels = await screen.findAllByTestId("repartition-label");
    // Libellé « sans axe » localisé (locale par défaut des tests = en) — jamais une entrée manquante.
    expect(labels[0]).toHaveTextContent("(none)");
  });

  it("interroge l'endpoint de répartition", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValue(response("0.00", [], []));
    renderCard();
    await waitFor(() =>
      expect(get).toHaveBeenCalledWith("/statistics/repartition", { params: { query: {} } }),
    );
  });

  it("signale une répartition partielle (complete=false)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      response("10.00", [{ label: "X", total: "10.00", count: 1 }], [], false),
    );
    renderCard();
    expect(await screen.findByTestId("repartition-incomplete")).toBeInTheDocument();
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderCard();
    expect(await screen.findByTestId("repartition-error")).toBeInTheDocument();
  });
});
