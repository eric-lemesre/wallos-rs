import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { LoginForm } from "./LoginForm";

/** @implements REQ-AUT-002 */

function renderForm() {
  return render(
    <I18nextProvider i18n={i18n}>
      <LoginForm />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("LoginForm", () => {
  it("affiche des libellés internationalisés", () => {
    renderForm();
    expect(screen.getByLabelText(i18n.t("login.email"))).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: i18n.t("login.submit") }),
    ).toBeInTheDocument();
  });

  it("ouvre la session puis affiche le compte courant via /me", async () => {
    const post = vi
      .spyOn(api, "POST")
      .mockResolvedValue({ response: new Response(null, { status: 200 }) } as never);
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValue({
        data: { email: "alice@example.com" },
        response: new Response(null, { status: 200 }),
      } as never);
    const user = userEvent.setup();
    renderForm();

    await user.type(screen.getByTestId("login-email"), "alice@example.com");
    await user.type(screen.getByTestId("login-password"), "correct horse battery staple");
    await user.click(screen.getByTestId("login-submit"));

    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    expect(get).toHaveBeenCalledWith("/me");
    expect(await screen.findByTestId("login-current-user")).toHaveTextContent(
      "alice@example.com",
    );
  });

  it("affiche un message générique quand les identifiants sont invalides", async () => {
    vi.spyOn(api, "POST").mockResolvedValue({
      response: new Response(null, { status: 401 }),
    } as never);
    const get = vi.spyOn(api, "GET");
    const user = userEvent.setup();
    renderForm();

    await user.type(screen.getByTestId("login-email"), "alice@example.com");
    await user.type(screen.getByTestId("login-password"), "wrong");
    await user.click(screen.getByTestId("login-submit"));

    expect(await screen.findByTestId("login-error")).toHaveTextContent(
      i18n.t("login.error"),
    );
    // Pas d'appel à /me si l'authentification échoue.
    expect(get).not.toHaveBeenCalled();
  });
});
