import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { SignupForm } from "./SignupForm";

/** @implements REQ-AUT-001 */

function renderForm() {
  return render(
    <I18nextProvider i18n={i18n}>
      <SignupForm />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("SignupForm", () => {
  it("affiche des libellés internationalisés, jamais de chaîne littérale", () => {
    renderForm();
    expect(screen.getByLabelText(i18n.t("signup.email"))).toBeInTheDocument();
    expect(screen.getByLabelText(i18n.t("signup.password"))).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: i18n.t("signup.submit") }),
    ).toBeInTheDocument();
  });

  it("refuse un mot de passe trop court sans appeler l'API", async () => {
    const post = vi.spyOn(api, "POST");
    const user = userEvent.setup();
    renderForm();

    await user.type(screen.getByTestId("signup-email"), "alice@example.com");
    await user.type(screen.getByTestId("signup-password"), "short");
    await user.click(screen.getByTestId("signup-submit"));

    expect(await screen.findByTestId("signup-password-error")).toHaveTextContent(
      i18n.t("signup.validation.passwordTooShort"),
    );
    expect(post).not.toHaveBeenCalled();
  });

  it("soumet un formulaire valide au contrat et affiche le succès", async () => {
    const post = vi
      .spyOn(api, "POST")
      .mockResolvedValue({ response: new Response(null, { status: 201 }) } as never);
    const user = userEvent.setup();
    renderForm();

    await user.type(screen.getByTestId("signup-email"), "alice@example.com");
    await user.type(
      screen.getByTestId("signup-password"),
      "correct horse battery staple",
    );
    await user.click(screen.getByTestId("signup-submit"));

    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    expect(post).toHaveBeenCalledWith("/accounts", {
      body: {
        email: "alice@example.com",
        password: "correct horse battery staple",
      },
    });
    expect(await screen.findByTestId("signup-success")).toBeInTheDocument();
  });
});
