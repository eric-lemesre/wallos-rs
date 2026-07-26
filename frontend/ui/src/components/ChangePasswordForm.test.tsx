import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { ChangePasswordForm } from "./ChangePasswordForm";

/** @implements REQ-AUT-007 */

const CURRENT = "correct horse battery staple";
const NEW = "totally fresh secret passphrase";

function renderForm() {
  return render(
    <I18nextProvider i18n={i18n}>
      <ChangePasswordForm />
    </I18nextProvider>,
  );
}

async function fillAndSubmit() {
  const user = userEvent.setup();
  await user.type(screen.getByTestId("change-password-current"), CURRENT);
  await user.type(screen.getByTestId("change-password-new"), NEW);
  await user.click(screen.getByTestId("change-password-submit"));
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ChangePasswordForm", () => {
  it("affiche des libellés internationalisés", () => {
    renderForm();
    expect(screen.getByLabelText(i18n.t("changePassword.current"))).toBeInTheDocument();
    expect(screen.getByLabelText(i18n.t("changePassword.new"))).toBeInTheDocument();
  });

  it("confirme le changement au succès (204)", async () => {
    const put = vi
      .spyOn(api, "PUT")
      .mockResolvedValue({ response: new Response(null, { status: 204 }) } as never);
    renderForm();
    await fillAndSubmit();

    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/password", {
        body: { current_password: CURRENT, new_password: NEW },
      }),
    );
    expect(await screen.findByTestId("change-password-success")).toHaveTextContent(
      i18n.t("changePassword.success"),
    );
  });

  it("signale un mot de passe actuel incorrect (403)", async () => {
    vi.spyOn(api, "PUT").mockResolvedValue({
      response: new Response(null, { status: 403 }),
    } as never);
    renderForm();
    await fillAndSubmit();

    expect(await screen.findByTestId("change-password-wrong-current")).toHaveTextContent(
      i18n.t("changePassword.wrongCurrent"),
    );
  });

  it("valide côté client : nouveau mot de passe trop court, aucun appel réseau", async () => {
    const put = vi.spyOn(api, "PUT");
    const user = userEvent.setup();
    renderForm();

    await user.type(screen.getByTestId("change-password-current"), CURRENT);
    await user.type(screen.getByTestId("change-password-new"), "short");
    await user.click(screen.getByTestId("change-password-submit"));

    expect(await screen.findByTestId("change-password-new-error")).toBeInTheDocument();
    expect(put).not.toHaveBeenCalled();
  });
});
