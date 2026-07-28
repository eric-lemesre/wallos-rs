import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { ReferenceCurrencySetting } from "./ReferenceCurrencySetting";

/** @implements REQ-CUR-001 */

function ok(currency: string) {
  return { data: { currency }, response: new Response(null, { status: 200 }) } as never;
}

function renderIt() {
  return render(
    <I18nextProvider i18n={i18n}>
      <ReferenceCurrencySetting />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ReferenceCurrencySetting", () => {
  it("affiche la devise de référence courante", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok("EUR"));
    renderIt();
    expect(await screen.findByTestId("reference-currency-current")).toHaveTextContent("EUR");
  });

  it("enregistre une nouvelle devise (PUT) et rafraîchit l'affichage", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok("EUR"));
    const put = vi.spyOn(api, "PUT").mockResolvedValue(ok("USD"));
    const user = userEvent.setup();
    renderIt();

    await screen.findByTestId("reference-currency-current");
    const input = screen.getByTestId("reference-currency-input");
    await user.clear(input);
    await user.type(input, "usd"); // saisie en minuscules -> normalisée en majuscules
    await user.click(screen.getByTestId("reference-currency-save"));

    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/settings/reference-currency", {
        body: { currency: "USD" },
      }),
    );
    expect(await screen.findByTestId("reference-currency-current")).toHaveTextContent("USD");
  });

  it("signale une erreur de mise à jour", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok("EUR"));
    vi.spyOn(api, "PUT").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderIt();

    await screen.findByTestId("reference-currency-current");
    await user.click(screen.getByTestId("reference-currency-save"));
    expect(await screen.findByTestId("reference-currency-error")).toBeInTheDocument();
  });
});
