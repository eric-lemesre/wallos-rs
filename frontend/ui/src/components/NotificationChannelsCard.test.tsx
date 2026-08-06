import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { NotificationChannelsCard } from "./NotificationChannelsCard";

/** @implements REQ-NOT-005 */

const CHANNEL = {
  id: "11111111-1111-1111-1111-111111111111",
  kind: "webhook",
  config: { url: "https://hooks.example.com/abc" },
  enabled: true,
};

function ok(data: unknown, status = 200) {
  return { data, response: new Response(null, { status }) } as never;
}

function renderCard() {
  return render(
    <I18nextProvider i18n={i18n}>
      <NotificationChannelsCard />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("NotificationChannelsCard", () => {
  it("liste les canaux configurés", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [CHANNEL] }));
    renderCard();
    expect(await screen.findByTestId("notification-channel-target")).toHaveTextContent(
      "https://hooks.example.com/abc",
    );
    expect(screen.getByTestId("notification-channel-kind")).toHaveTextContent("webhook");
  });

  it("affiche l'état vide", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    renderCard();
    expect(await screen.findByTestId("notification-channels-empty")).toBeInTheDocument();
  });

  it("ajoute un webhook puis rafraîchit", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValueOnce(ok({ channels: [] }))
      .mockResolvedValueOnce(ok({ channels: [CHANNEL] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok(CHANNEL, 201));
    const user = userEvent.setup();
    renderCard();

    await user.type(
      await screen.findByTestId("notification-channel-url"),
      "https://hooks.example.com/abc",
    );
    await user.click(screen.getByTestId("notification-channel-add"));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: { kind: "webhook", config: { url: "https://hooks.example.com/abc" } },
      }),
    );
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("notification-channel-target")).toBeInTheDocument();
  });

  it("signale une URL refusée (SSRF)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    vi.spyOn(api, "POST").mockResolvedValue(ok(undefined, 422));
    const user = userEvent.setup();
    renderCard();

    await user.type(
      await screen.findByTestId("notification-channel-url"),
      "http://localhost/hook",
    );
    await user.click(screen.getByTestId("notification-channel-add"));
    expect(await screen.findByTestId("notification-channel-rejected")).toBeInTheDocument();
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok(undefined, 500));
    renderCard();
    expect(await screen.findByTestId("notification-channels-error")).toBeInTheDocument();
  });
});
