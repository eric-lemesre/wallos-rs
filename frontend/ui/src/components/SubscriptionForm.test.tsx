import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { SubscriptionForm } from "./SubscriptionForm";

/** @implements REQ-SUB-002 */

function renderForm() {
  return render(
    <I18nextProvider i18n={i18n}>
      <SubscriptionForm />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

/** Mock du chargement des catégories (peuple le sélecteur au montage). */
function mockCategories(categories: { id: string; name: string }[]) {
  return vi.spyOn(api, "GET").mockResolvedValue({
    data: categories,
    response: new Response(null, { status: 200 }),
  } as never);
}

describe("SubscriptionForm", () => {
  it("crée un abonnement et affiche la prochaine échéance", async () => {
    mockCategories([]);
    vi.spyOn(api, "POST").mockResolvedValue({
      data: { id: "x", name: "Netflix", amount: "9.99", currency: "EUR", cycle: { unit: "month", interval: 1 }, first_payment: "2030-01-31", active: true, next_payment: "2030-01-31" },
      response: new Response(null, { status: 201 }),
    } as never);
    const user = userEvent.setup();
    renderForm();
    await user.type(screen.getByTestId("sub-name"), "Netflix");
    await user.click(screen.getByTestId("sub-submit"));
    expect(await screen.findByTestId("sub-success")).toHaveTextContent("Jan 31, 2030");
  });

  it("affiche l'erreur de validation par champ", async () => {
    mockCategories([]);
    vi.spyOn(api, "POST").mockResolvedValue({
      error: { type: "about:blank", title: "Unprocessable Entity", status: 422, detail: "amount: montant négatif" },
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderForm();
    await user.click(screen.getByTestId("sub-submit"));
    expect(await screen.findByTestId("sub-error")).toHaveTextContent("amount");
  });

  it("rattache l'abonnement à une catégorie sélectionnée (REQ-CAT-001)", async () => {
    mockCategories([{ id: "cat-1", name: "Streaming" }]);
    const post = vi.spyOn(api, "POST").mockResolvedValue({
      data: { id: "x", name: "Netflix", amount: "9.99", currency: "EUR", cycle: { unit: "month", interval: 1 }, first_payment: "2030-01-31", active: true, next_payment: "2030-01-31" },
      response: new Response(null, { status: 201 }),
    } as never);
    const user = userEvent.setup();
    renderForm();
    // Le sélecteur est peuplé depuis GET /categories ; on choisit la catégorie par son libellé.
    await user.selectOptions(await screen.findByTestId("sub-category"), "cat-1");
    await user.type(screen.getByTestId("sub-name"), "Netflix");
    await user.click(screen.getByTestId("sub-submit"));
    // Le corps du POST porte la catégorie sélectionnée.
    expect(post).toHaveBeenCalledWith(
      "/subscriptions",
      expect.objectContaining({ body: expect.objectContaining({ category: "cat-1" }) }),
    );
  });
});
