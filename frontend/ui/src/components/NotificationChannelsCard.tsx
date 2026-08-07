import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type NotificationChannelDto = components["schemas"]["NotificationChannelDto"];
type TestChannelResponse = components["schemas"]["TestNotificationChannelResponse"];
type NotificationDeliveryDto = components["schemas"]["NotificationDeliveryDto"];

type ChannelKind = "webhook" | "email" | "telegram" | "discord" | "gotify" | "pushover";

/** Résultat du dernier envoi de test, rattaché au canal testé. */
type TestOutcome = { channelId: string; response: TestChannelResponse | null };

/**
 * Canaux de notification du foyer : liste, ajout et suppression. Webhook générique (REQ-NOT-005),
 * e-mail SMTP (REQ-NOT-003) et messageries Telegram, Discord, Gotify, Pushover (REQ-NOT-004),
 * tous portés par la même abstraction serveur. Toute URL fournie (webhook, Discord, Gotify) est
 * validée côté serveur contre la falsification de requête (SSRF) ; les secrets (mot de passe SMTP,
 * jetons, clé utilisateur) ne sont jamais renvoyés (redactés). Isolée par le serveur (§9). Aucune
 * chaîne d'affichage en dur (REQ-I18N-002).
 *
 * Chaque canal offre un envoi de **test** (REQ-NOT-006) : le résultat est affiché avec un
 * diagnostic localisé à partir d'un code stable renvoyé par le serveur (jamais l'erreur brute).
 * Les livraisons en difficulté — réessai planifié ou **abandon** — sont affichées ici même
 * (REQ-NOT-007 critère #2 : visible dans l'interface, pas seulement dans les journaux).
 *
 * @implements REQ-NOT-005
 * @implements REQ-NOT-003
 * @implements REQ-NOT-004
 * @implements REQ-NOT-006
 * @implements REQ-NOT-007
 */
export function NotificationChannelsCard() {
  const { t } = useTranslation();
  const [channels, setChannels] = useState<NotificationChannelDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [rejected, setRejected] = useState(false);
  const [kind, setKind] = useState<ChannelKind>("webhook");
  const [url, setUrl] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("587");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [from, setFrom] = useState("");
  const [botToken, setBotToken] = useState("");
  const [chatId, setChatId] = useState("");
  const [token, setToken] = useState("");
  const [userKey, setUserKey] = useState("");
  const [botUsername, setBotUsername] = useState("");
  const [botAvatarUrl, setBotAvatarUrl] = useState("");
  const [testOutcome, setTestOutcome] = useState<TestOutcome | null>(null);
  const [testing, setTesting] = useState(false);
  const [deliveries, setDeliveries] = useState<NotificationDeliveryDto[]>([]);

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/notifications/channels");
    if (response.ok && data) {
      setChannels(data.channels);
      setFailed(false);
    } else {
      setFailed(true);
    }
    // Livraisons en difficulté (REQ-NOT-007) : les abandons doivent être VISIBLES ici,
    // pas seulement dans les journaux du serveur.
    const failures = await api.GET("/notifications/deliveries");
    if (failures.response.ok && failures.data) {
      setDeliveries(failures.data.deliveries ?? []);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  /** Configuration à enregistrer selon le type de canal (schémas validés côté serveur). */
  function buildConfig(): Record<string, unknown> {
    switch (kind) {
      case "webhook":
        return { url };
      case "email":
        return { host, port: Number.parseInt(port, 10), username, password, from };
      case "telegram":
        return { bot_token: botToken, chat_id: chatId };
      case "discord":
        return {
          url,
          ...(botUsername ? { username: botUsername } : {}),
          ...(botAvatarUrl ? { avatar_url: botAvatarUrl } : {}),
        };
      case "gotify":
        return { url, token };
      case "pushover":
        return { user_key: userKey, token };
    }
  }

  async function addChannel() {
    const { response } = await api.POST("/notifications/channels", {
      body: { kind, config: buildConfig() },
    });
    if (!response.ok) {
      // Configuration refusée (URL SSRF, adresse illisible, champ manquant) : signalé, liste inchangée.
      setRejected(true);
      return;
    }
    setRejected(false);
    setUrl("");
    setHost("");
    setUsername("");
    setPassword("");
    setFrom("");
    setBotToken("");
    setChatId("");
    setToken("");
    setUserKey("");
    setBotUsername("");
    setBotAvatarUrl("");
    await refresh();
  }

  async function remove(id: string) {
    await api.DELETE("/notifications/channels/{id}", { params: { path: { id } } });
    await refresh();
  }

  async function testChannel(id: string) {
    // Un seul test à la fois : l'envoi est une requête sortante côté serveur (revue NOT-006 F2).
    if (testing) {
      return;
    }
    setTesting(true);
    try {
      const { data, response } = await api.POST("/notifications/channels/{id}/test", {
        params: { path: { id } },
      });
      setTestOutcome({ channelId: id, response: response.ok && data ? data : null });
    } finally {
      setTesting(false);
    }
  }

  /** Message localisé du résultat d'un test (code stable → clé i18n). */
  function testMessage(result: TestChannelResponse | null): string {
    if (!result) {
      return t("notificationChannels.testFailed");
    }
    switch (result.code) {
      case "sent":
        return t("notificationChannels.testSent");
      case "http-status":
        return t("notificationChannels.testHttpStatus", { status: result.http_status ?? "?" });
      case "timeout":
        return t("notificationChannels.testTimeout");
      case "connection-failed":
        return t("notificationChannels.testConnectionFailed");
      case "smtp-failed":
        return t("notificationChannels.testSmtpFailed");
      default:
        return t("notificationChannels.testSendFailed");
    }
  }

  /**
   * Libellé de la cible d'un canal : URL (webhook, Discord, Gotify), expéditeur via hôte SMTP
   * (e-mail), conversation (Telegram). Pushover n'a aucune cible non secrète à afficher.
   */
  function target(channel: NotificationChannelDto): string {
    const config = channel.config as {
      url?: unknown;
      from?: unknown;
      host?: unknown;
      chat_id?: unknown;
    };
    switch (channel.kind) {
      case "webhook":
      case "discord":
      case "gotify":
        return typeof config?.url === "string" ? config.url : "";
      case "email": {
        const fromAddr = typeof config?.from === "string" ? config.from : "";
        const hostName = typeof config?.host === "string" ? config.host : "";
        return `${fromAddr} — ${hostName}`;
      }
      case "telegram":
        return typeof config?.chat_id === "string" ? config.chat_id : "";
      default:
        return "";
    }
  }

  /** Libellé localisé du statut d'une livraison en difficulté (REQ-NOT-007). */
  function deliveryStatus(delivery: NotificationDeliveryDto): string {
    return delivery.status === "abandoned"
      ? t("notificationChannels.deliveryAbandoned", { attempts: delivery.attempts })
      : t("notificationChannels.deliveryPending", { attempts: delivery.attempts });
  }

  /** L'URL saisie sert le webhook générique, le webhook Discord et le serveur Gotify. */
  const needsUrl = kind === "webhook" || kind === "discord" || kind === "gotify";

  return (
    <section data-testid="notification-channels-card" aria-label={t("notificationChannels.title")}>
      <h2>{t("notificationChannels.title")}</h2>

      {failed && (
        <p data-testid="notification-channels-error" role="alert">
          {t("notificationChannels.loadError")}
        </p>
      )}

      {rejected && (
        <p data-testid="notification-channel-rejected" role="alert">
          {t("notificationChannels.rejected")}
        </p>
      )}

      <div>
        <select
          data-testid="notification-channel-type"
          aria-label={t("notificationChannels.typeLabel")}
          value={kind}
          onChange={(e) => setKind(e.target.value as ChannelKind)}
        >
          <option value="webhook">{t("notificationChannels.webhook")}</option>
          <option value="email">{t("notificationChannels.email")}</option>
          <option value="telegram">{t("notificationChannels.telegram")}</option>
          <option value="discord">{t("notificationChannels.discord")}</option>
          <option value="gotify">{t("notificationChannels.gotify")}</option>
          <option value="pushover">{t("notificationChannels.pushover")}</option>
        </select>

        {needsUrl && (
          <input
            data-testid="notification-channel-url"
            aria-label={t("notificationChannels.urlLabel")}
            placeholder={t("notificationChannels.urlPlaceholder")}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
        )}

        {kind === "email" && (
          <span data-testid="notification-channel-email-fields">
            <input
              data-testid="notification-channel-host"
              aria-label={t("notificationChannels.host")}
              value={host}
              onChange={(e) => setHost(e.target.value)}
            />
            <input
              data-testid="notification-channel-port"
              aria-label={t("notificationChannels.port")}
              inputMode="numeric"
              value={port}
              onChange={(e) => setPort(e.target.value)}
            />
            <input
              data-testid="notification-channel-username"
              aria-label={t("notificationChannels.username")}
              value={username}
              onChange={(e) => setUsername(e.target.value)}
            />
            <input
              data-testid="notification-channel-password"
              aria-label={t("notificationChannels.password")}
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />
            <input
              data-testid="notification-channel-from"
              aria-label={t("notificationChannels.from")}
              value={from}
              onChange={(e) => setFrom(e.target.value)}
            />
          </span>
        )}

        {kind === "telegram" && (
          <span data-testid="notification-channel-telegram-fields">
            <input
              data-testid="notification-channel-bot-token"
              aria-label={t("notificationChannels.botToken")}
              type="password"
              value={botToken}
              onChange={(e) => setBotToken(e.target.value)}
            />
            <input
              data-testid="notification-channel-chat-id"
              aria-label={t("notificationChannels.chatId")}
              value={chatId}
              onChange={(e) => setChatId(e.target.value)}
            />
          </span>
        )}

        {kind === "discord" && (
          <span data-testid="notification-channel-discord-fields">
            <input
              data-testid="notification-channel-bot-username"
              aria-label={t("notificationChannels.botUsername")}
              value={botUsername}
              onChange={(e) => setBotUsername(e.target.value)}
            />
            <input
              data-testid="notification-channel-bot-avatar"
              aria-label={t("notificationChannels.botAvatarUrl")}
              value={botAvatarUrl}
              onChange={(e) => setBotAvatarUrl(e.target.value)}
            />
          </span>
        )}

        {(kind === "gotify" || kind === "pushover") && (
          <input
            data-testid="notification-channel-token"
            aria-label={t("notificationChannels.token")}
            type="password"
            value={token}
            onChange={(e) => setToken(e.target.value)}
          />
        )}

        {kind === "pushover" && (
          <input
            data-testid="notification-channel-user-key"
            aria-label={t("notificationChannels.userKey")}
            type="password"
            value={userKey}
            onChange={(e) => setUserKey(e.target.value)}
          />
        )}

        <button
          type="button"
          data-testid="notification-channel-add"
          onClick={() => void addChannel()}
        >
          {t("notificationChannels.add")}
        </button>
      </div>

      {deliveries.length > 0 && (
        <div data-testid="notification-deliveries">
          <h3>{t("notificationChannels.deliveriesTitle")}</h3>
          <ul>
            {deliveries.map((delivery) => (
              <li
                key={delivery.id}
                data-testid="notification-delivery-row"
                data-status={delivery.status}
              >
                <span data-testid="notification-delivery-kind">{delivery.channel_kind}</span>
                <span data-testid="notification-delivery-date">{delivery.as_of}</span>
                <span data-testid="notification-delivery-status">{deliveryStatus(delivery)}</span>
              </li>
            ))}
          </ul>
        </div>
      )}

      {channels.length === 0 ? (
        <p data-testid="notification-channels-empty">{t("notificationChannels.empty")}</p>
      ) : (
        <ul>
          {channels.map((channel) => (
            <li key={channel.id} data-testid="notification-channel-row">
              <span data-testid="notification-channel-kind">{channel.kind}</span>
              <span data-testid="notification-channel-target">{target(channel)}</span>
              <button
                type="button"
                data-testid={`notification-channel-test-${channel.id}`}
                disabled={testing}
                onClick={() => void testChannel(channel.id)}
              >
                {t("notificationChannels.test")}
              </button>
              <button
                type="button"
                data-testid={`notification-channel-delete-${channel.id}`}
                onClick={() => void remove(channel.id)}
              >
                {t("notificationChannels.delete")}
              </button>
              {testOutcome?.channelId === channel.id && (
                <span
                  data-testid="notification-channel-test-result"
                  role="status"
                  data-ok={testOutcome.response?.ok === true}
                >
                  {testMessage(testOutcome.response)}
                </span>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
