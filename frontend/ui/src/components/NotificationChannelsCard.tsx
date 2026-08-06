import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type NotificationChannelDto = components["schemas"]["NotificationChannelDto"];

/**
 * Canaux de notification du foyer (REQ-NOT-005 webhook, REQ-NOT-003 e-mail) : liste, ajout et
 * suppression. Pour un webhook, l'URL est validée côté serveur contre la falsification de requête
 * (SSRF) ; pour un e-mail, la configuration SMTP est validée (une adresse d'expéditeur illisible est
 * refusée). Le mot de passe SMTP n'est jamais renvoyé (redacté). Isolée par le serveur (§9). Aucune
 * chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-NOT-005
 * @implements REQ-NOT-003
 */
export function NotificationChannelsCard() {
  const { t } = useTranslation();
  const [channels, setChannels] = useState<NotificationChannelDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [rejected, setRejected] = useState(false);
  const [kind, setKind] = useState<"webhook" | "email">("webhook");
  const [url, setUrl] = useState("");
  const [host, setHost] = useState("");
  const [port, setPort] = useState("587");
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [from, setFrom] = useState("");

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/notifications/channels");
    if (response.ok && data) {
      setChannels(data.channels);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function addChannel() {
    const config =
      kind === "webhook"
        ? { url }
        : { host, port: Number.parseInt(port, 10), username, password, from };
    const { response } = await api.POST("/notifications/channels", {
      body: { kind, config },
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
    await refresh();
  }

  async function remove(id: string) {
    await api.DELETE("/notifications/channels/{id}", { params: { path: { id } } });
    await refresh();
  }

  /** Libellé de la cible d'un canal (URL de webhook, ou expéditeur via hôte SMTP pour un e-mail). */
  function target(channel: NotificationChannelDto): string {
    const config = channel.config as { url?: unknown; from?: unknown; host?: unknown };
    if (channel.kind === "webhook") {
      return typeof config?.url === "string" ? config.url : "";
    }
    const fromAddr = typeof config?.from === "string" ? config.from : "";
    const hostName = typeof config?.host === "string" ? config.host : "";
    return `${fromAddr} — ${hostName}`;
  }

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
          onChange={(e) => setKind(e.target.value as "webhook" | "email")}
        >
          <option value="webhook">{t("notificationChannels.webhook")}</option>
          <option value="email">{t("notificationChannels.email")}</option>
        </select>

        {kind === "webhook" ? (
          <input
            data-testid="notification-channel-url"
            aria-label={t("notificationChannels.urlLabel")}
            placeholder={t("notificationChannels.urlPlaceholder")}
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
        ) : (
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

        <button
          type="button"
          data-testid="notification-channel-add"
          onClick={() => void addChannel()}
        >
          {t("notificationChannels.add")}
        </button>
      </div>

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
                data-testid={`notification-channel-delete-${channel.id}`}
                onClick={() => void remove(channel.id)}
              >
                {t("notificationChannels.delete")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
