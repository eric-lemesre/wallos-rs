import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { PaymentMethodsList } from "./PaymentMethodsList";

/** @implements REQ-SUB-011 */

const PM = { id: "11111111-1111-1111-1111-111111111111", name: "Carte de crédit" };

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <PaymentMethodsList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("PaymentMethodsList", () => {
  it("liste les moyens de paiement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([PM]));
    renderList();
    expect(await screen.findByTestId("payment-method-name")).toHaveTextContent("Carte de crédit");
  });

  it("crée un moyen de paiement puis rafraîchit", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValueOnce(ok([])).mockResolvedValueOnce(ok([PM]));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok(PM));
    const user = userEvent.setup();
    renderList();

    await user.type(screen.getByTestId("payment-method-new-name"), "Carte de crédit");
    await user.click(screen.getByTestId("payment-method-create"));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/payment-methods", { body: { name: "Carte de crédit" } }),
    );
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("payment-method-name")).toHaveTextContent("Carte de crédit");
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderList();
    expect(await screen.findByTestId("payment-methods-error")).toBeInTheDocument();
  });
});
