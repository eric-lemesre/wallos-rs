import { render, screen } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { CurrenciesList } from "./CurrenciesList";

/** @implements REQ-CUR-007 */

const EUR = { code: "EUR", symbol: "€", name: "Euro", decimals: 2 };
const JPY = { code: "JPY", symbol: "¥", name: "Japanese Yen", decimals: 0 };

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <CurrenciesList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("CurrenciesList", () => {
  it("affiche le référentiel avec symbole, code, nom et décimales", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([EUR, JPY]));
    renderList();

    expect(await screen.findAllByTestId("currency-row")).toHaveLength(2);
    expect(screen.getAllByTestId("currency-code").map((n) => n.textContent)).toEqual(["EUR", "JPY"]);
    expect(screen.getAllByTestId("currency-symbol").map((n) => n.textContent)).toEqual(["€", "¥"]);
    // Les décimales dépendent de la devise (2 pour EUR, 0 pour JPY).
    const decimals = screen.getAllByTestId("currency-decimals").map((n) => n.textContent);
    expect(decimals[0]).toContain("2");
    expect(decimals[1]).toContain("0");
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderList();
    expect(await screen.findByTestId("currencies-error")).toHaveTextContent(
      i18n.t("currencies.loadError"),
    );
  });
});
