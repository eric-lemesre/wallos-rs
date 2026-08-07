import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { NotificationChannelsCard } from "./NotificationChannelsCard";

/** @implements REQ-NOT-005 */
/** @implements REQ-NOT-004 */
/** @implements REQ-NOT-006 */
/** @implements REQ-NOT-007 */

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
    // Le rafraîchissement charge les canaux ET les livraisons en difficulté (REQ-NOT-007) :
    // mock par chemin, avec une liste de canaux vide avant l'ajout puis remplie après.
    let added = false;
    const get = vi.spyOn(api, "GET").mockImplementation(((path: string) =>
      path === "/notifications/deliveries"
        ? Promise.resolve(ok({ deliveries: [] }))
        : Promise.resolve(ok({ channels: added ? [CHANNEL] : [] }))) as never);
    const post = vi.spyOn(api, "POST").mockImplementation((() => {
      added = true;
      return Promise.resolve(ok(CHANNEL, 201));
    }) as never);
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
    await waitFor(() => expect(get).toHaveBeenCalledTimes(4)); // 2 rafraîchissements × (canaux + livraisons)
    expect(await screen.findByTestId("notification-channel-target")).toBeInTheDocument();
  });

  it("ajoute un canal e-mail avec sa configuration SMTP", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ...CHANNEL, kind: "email" }, 201));
    const user = userEvent.setup();
    renderCard();

    await user.selectOptions(await screen.findByTestId("notification-channel-type"), "email");
    await user.type(screen.getByTestId("notification-channel-host"), "smtp.example.com");
    await user.clear(screen.getByTestId("notification-channel-port"));
    await user.type(screen.getByTestId("notification-channel-port"), "587");
    await user.type(screen.getByTestId("notification-channel-username"), "alice");
    await user.type(screen.getByTestId("notification-channel-password"), "s3cr3t");
    await user.type(screen.getByTestId("notification-channel-from"), "wallos@example.com");
    await user.click(screen.getByTestId("notification-channel-add"));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: {
          kind: "email",
          config: {
            host: "smtp.example.com",
            port: 587,
            username: "alice",
            password: "s3cr3t",
            from: "wallos@example.com",
          },
        },
      }),
    );
  });

  it("ajoute un canal Telegram (jeton de bot + conversation)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ...CHANNEL, kind: "telegram" }, 201));
    const user = userEvent.setup();
    renderCard();

    await user.selectOptions(await screen.findByTestId("notification-channel-type"), "telegram");
    await user.type(screen.getByTestId("notification-channel-bot-token"), "123:abc");
    await user.type(screen.getByTestId("notification-channel-chat-id"), "42");
    await user.click(screen.getByTestId("notification-channel-add"));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: { kind: "telegram", config: { bot_token: "123:abc", chat_id: "42" } },
      }),
    );
  });

  it("ajoute un canal Discord avec nom et avatar de bot optionnels", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ...CHANNEL, kind: "discord" }, 201));
    const user = userEvent.setup();
    renderCard();

    await user.selectOptions(await screen.findByTestId("notification-channel-type"), "discord");
    await user.type(
      screen.getByTestId("notification-channel-url"),
      "https://discord.com/api/webhooks/1/x",
    );
    await user.type(screen.getByTestId("notification-channel-bot-username"), "Wallos");
    await user.click(screen.getByTestId("notification-channel-add"));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: {
          kind: "discord",
          config: { url: "https://discord.com/api/webhooks/1/x", username: "Wallos" },
        },
      }),
    );
  });

  it("ajoute un canal Gotify (serveur + jeton)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ...CHANNEL, kind: "gotify" }, 201));
    const user = userEvent.setup();
    renderCard();

    await user.selectOptions(await screen.findByTestId("notification-channel-type"), "gotify");
    await user.type(screen.getByTestId("notification-channel-url"), "https://gotify.example.com");
    await user.type(screen.getByTestId("notification-channel-token"), "app-token");
    await user.click(screen.getByTestId("notification-channel-add"));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: { kind: "gotify", config: { url: "https://gotify.example.com", token: "app-token" } },
      }),
    );
  });

  it("ajoute un canal Pushover (clé utilisateur + jeton)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ...CHANNEL, kind: "pushover" }, 201));
    const user = userEvent.setup();
    renderCard();

    await user.selectOptions(await screen.findByTestId("notification-channel-type"), "pushover");
    await user.type(screen.getByTestId("notification-channel-token"), "tok");
    await user.type(screen.getByTestId("notification-channel-user-key"), "uk");
    await user.click(screen.getByTestId("notification-channel-add"));

    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels", {
        body: { kind: "pushover", config: { user_key: "uk", token: "tok" } },
      }),
    );
  });

  it("affiche la conversation d'un canal Telegram comme cible", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(
      ok({
        channels: [
          {
            ...CHANNEL,
            kind: "telegram",
            config: { bot_token: "<redacted>", chat_id: "42" },
          },
        ],
      }),
    );
    renderCard();
    expect(await screen.findByTestId("notification-channel-target")).toHaveTextContent("42");
    expect(screen.getByTestId("notification-channel-kind")).toHaveTextContent("telegram");
  });

  it("signale une configuration refusée (SSRF / SMTP)", async () => {
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

  it("teste un canal et affiche le succès", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [CHANNEL] }));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok({ ok: true, code: "sent" }));
    const user = userEvent.setup();
    renderCard();

    await user.click(await screen.findByTestId(`notification-channel-test-${CHANNEL.id}`));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/notifications/channels/{id}/test", {
        params: { path: { id: CHANNEL.id } },
      }),
    );
    const result = await screen.findByTestId("notification-channel-test-result");
    expect(result).toHaveAttribute("data-ok", "true");
  });

  it("affiche le diagnostic d'un test en échec (statut HTTP de la cible)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok({ channels: [CHANNEL] }));
    vi.spyOn(api, "POST").mockResolvedValue(
      ok({ ok: false, code: "http-status", http_status: 500 }),
    );
    const user = userEvent.setup();
    renderCard();

    await user.click(await screen.findByTestId(`notification-channel-test-${CHANNEL.id}`));
    const result = await screen.findByTestId("notification-channel-test-result");
    expect(result).toHaveAttribute("data-ok", "false");
    expect(result).toHaveTextContent("500");
  });

  it("affiche un abandon de livraison (REQ-NOT-007 : visible, pas seulement journalisé)", async () => {
    vi.spyOn(api, "GET").mockImplementation(((path: string) =>
      path === "/notifications/deliveries"
        ? Promise.resolve(
            ok({
              deliveries: [
                {
                  id: "22222222-2222-2222-2222-222222222222",
                  channel_id: CHANNEL.id,
                  channel_kind: "webhook",
                  as_of: "2026-08-07",
                  attempts: 5,
                  status: "abandoned",
                  last_code: "connection-failed",
                },
              ],
            }),
          )
        : Promise.resolve(ok({ channels: [CHANNEL] }))) as never);
    renderCard();

    const row = await screen.findByTestId("notification-delivery-row");
    expect(row).toHaveAttribute("data-status", "abandoned");
    expect(screen.getByTestId("notification-delivery-status")).toHaveTextContent("5");
    expect(screen.getByTestId("notification-delivery-kind")).toHaveTextContent("webhook");
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok(undefined, 500));
    renderCard();
    expect(await screen.findByTestId("notification-channels-error")).toBeInTheDocument();
  });
});
