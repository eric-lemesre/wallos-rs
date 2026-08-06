import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { PayersList } from "./PayersList";

/** @implements REQ-SUB-017 */

const PAYER = { id: "11111111-1111-1111-1111-111111111111", name: "Alex" };

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <PayersList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("PayersList", () => {
  it("liste les payeurs", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([PAYER]));
    renderList();
    expect(await screen.findByTestId("payer-name")).toHaveTextContent("Alex");
  });

  it("crée un payeur puis rafraîchit", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValueOnce(ok([])).mockResolvedValueOnce(ok([PAYER]));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok(PAYER));
    const user = userEvent.setup();
    renderList();

    await user.type(screen.getByTestId("payer-new-name"), "Alex");
    await user.click(screen.getByTestId("payer-create"));
    // En ligne : POST direct avec un id généré côté client (REQ-SYN-001) + le nom.
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/payers", {
        body: expect.objectContaining({ name: "Alex" }),
      }),
    );
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("payer-name")).toHaveTextContent("Alex");
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderList();
    expect(await screen.findByTestId("payers-error")).toBeInTheDocument();
  });
});
