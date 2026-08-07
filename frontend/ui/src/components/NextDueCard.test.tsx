import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { NextDueCard } from "./NextDueCard";

/** @implements REQ-SUB-012 */

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <NextDueCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("NextDueCard", () => {
  it("affiche la prochaine échéance (31 janv -> 28 févr)", async () => {
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ next_payment: "2025-02-28" }));
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("nextdue-compute"));
    expect(await screen.findByTestId("nextdue-result")).toHaveTextContent("Feb 28, 2025");
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/schedule/next-due", {
        body: { first_payment: "2025-01-31", cycle: { unit: "month", interval: 1 }, after: "2025-01-31" },
      }),
    );
  });

  it("signale une erreur (ex. 422)", async () => {
    vi.spyOn(api, "POST").mockResolvedValue({ data: undefined, response: new Response(null, { status: 422 }) } as never);
    const user = userEvent.setup();
    renderCard();
    await user.click(screen.getByTestId("nextdue-compute"));
    expect(await screen.findByTestId("nextdue-error")).toHaveTextContent(i18n.t("nextDue.error"));
  });
});
