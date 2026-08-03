import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { ImportExportCard } from "./ImportExportCard";

/** @implements REQ-SUB-016 */

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <ImportExportCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ImportExportCard", () => {
  it("exporte et affiche l'enveloppe JSON", async () => {
    const bundle = {
      version: 1,
      reference_currency: "USD",
      categories: [{ id: "c1", name: "Streaming" }],
      payment_methods: [],
      subscriptions: [],
    };
    vi.spyOn(api, "GET").mockResolvedValue({
      data: bundle,
      response: new Response(null, { status: 200 }),
    } as never);

    renderCard();
    await userEvent.click(screen.getByTestId("export-button"));

    await waitFor(() => {
      expect((screen.getByTestId("export-output") as HTMLTextAreaElement).value).toContain(
        "Streaming",
      );
    });
  });

  it("importe et affiche le rapport (créées + rejetées)", async () => {
    vi.spyOn(api, "POST").mockResolvedValue({
      data: {
        imported: { categories: 1, payment_methods: 0, subscriptions: 1 },
        rejected: [{ kind: "subscription", reference: "Bad", reason: "currency: hors référentiel" }],
      },
      response: new Response(null, { status: 200 }),
    } as never);

    renderCard();
    await userEvent.type(
      screen.getByTestId("import-input"),
      '{{"version":1}',
    );
    await userEvent.click(screen.getByTestId("import-button"));

    await waitFor(() => {
      expect(screen.getByTestId("import-rejected")).toHaveTextContent("Bad");
    });
    expect(screen.getByTestId("import-rejected-count")).toHaveTextContent("1");
  });

  it("signale un JSON invalide sans appeler l'API", async () => {
    const post = vi.spyOn(api, "POST");
    renderCard();
    await userEvent.type(screen.getByTestId("import-input"), "pas du json");
    await userEvent.click(screen.getByTestId("import-button"));

    await waitFor(() => {
      expect(screen.getByTestId("import-export-error")).toBeInTheDocument();
    });
    expect(post).not.toHaveBeenCalled();
  });
});
