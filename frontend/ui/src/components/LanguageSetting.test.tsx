import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { LanguageSetting } from "./LanguageSetting";

/** @implements REQ-I18N-001 */

function ok(language: string | null) {
  const data = language === null ? {} : { language };
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderIt() {
  return render(
    <I18nextProvider i18n={i18n}>
      <LanguageSetting />
    </I18nextProvider>,
  );
}

afterEach(async () => {
  vi.restoreAllMocks();
  await i18n.changeLanguage("en"); // isole les tests (i18n est un singleton)
});

describe("LanguageSetting", () => {
  it("applique la langue persistée au chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok("fr"));
    renderIt();
    await waitFor(() => expect(i18n.language).toBe("fr"));
    expect(screen.getByTestId("language-current")).toHaveTextContent("fr");
  });

  it("applique la langue système si supportée quand aucune langue n'est persistée (acceptance #2)", async () => {
    Object.defineProperty(navigator, "language", { value: "fr-FR", configurable: true });
    vi.spyOn(api, "GET").mockResolvedValue(ok(null));
    renderIt();
    await waitFor(() => expect(i18n.language).toBe("fr"));
    Object.defineProperty(navigator, "language", { value: "en-US", configurable: true });
  });

  it("enregistre un choix (PUT) et applique la langue immédiatement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok(null));
    const put = vi.spyOn(api, "PUT").mockResolvedValue(ok("fr"));
    const user = userEvent.setup();
    renderIt();

    await screen.findByTestId("language-select");
    await user.selectOptions(screen.getByTestId("language-select"), "fr");
    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/settings/language", { body: { language: "fr" } }),
    );
    await waitFor(() => expect(i18n.language).toBe("fr"));
  });

  it("signale une erreur de mise à jour", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok(null));
    vi.spyOn(api, "PUT").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 422 }),
    } as never);
    const user = userEvent.setup();
    renderIt();

    await screen.findByTestId("language-select");
    await user.selectOptions(screen.getByTestId("language-select"), "fr");
    expect(await screen.findByTestId("language-error")).toBeInTheDocument();
  });
});
