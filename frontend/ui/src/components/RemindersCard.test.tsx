import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { RemindersCard } from "./RemindersCard";

/** @implements REQ-NOT-001 */

function setting(leadDays: number) {
  return { data: { lead_days: leadDays }, response: new Response(null, { status: 200 }) } as never;
}

function reminders(items: { subscription_id: string; name: string; due_date: string; days_until: number }[]) {
  return {
    data: { as_of: "2026-08-06", reminders: items },
    response: new Response(null, { status: 200 }),
  } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <RemindersCard />
    </I18nextProvider>,
  );
}

afterEach(() => vi.restoreAllMocks());

describe("RemindersCard", () => {
  it("affiche le délai et les rappels du jour", async () => {
    vi.spyOn(api, "GET").mockImplementation((path: string) =>
      path === "/settings/reminder"
        ? setting(1)
        : (reminders([{ subscription_id: "s1", name: "Netflix", due_date: "2026-08-07", days_until: 1 }]) as never),
    );
    renderCard();

    expect(await screen.findByTestId("reminder-name")).toHaveTextContent("Netflix");
    expect(screen.getByTestId("reminders-lead-current")).toBeInTheDocument();
  });

  it("indique l'absence de rappel", async () => {
    vi.spyOn(api, "GET").mockImplementation((path: string) =>
      path === "/settings/reminder" ? setting(2) : (reminders([]) as never),
    );
    renderCard();
    expect(await screen.findByTestId("reminders-empty")).toBeInTheDocument();
  });

  it("enregistre un nouveau délai (PUT)", async () => {
    vi.spyOn(api, "GET").mockImplementation((path: string) =>
      path === "/settings/reminder" ? setting(1) : (reminders([]) as never),
    );
    const put = vi.spyOn(api, "PUT").mockResolvedValue(setting(5));
    const user = userEvent.setup();
    renderCard();

    await screen.findByTestId("reminders-empty");
    const input = screen.getByTestId("reminders-lead-input");
    await user.clear(input);
    await user.type(input, "5");
    await user.click(screen.getByTestId("reminders-save"));

    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/settings/reminder", { body: { lead_days: 5 } }),
    );
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderCard();
    expect(await screen.findByTestId("reminders-error")).toBeInTheDocument();
  });
});
