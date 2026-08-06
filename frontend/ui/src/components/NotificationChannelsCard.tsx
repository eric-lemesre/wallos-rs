import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type NotificationChannelDto = components["schemas"]["NotificationChannelDto"];

/**
 * Canaux de notification du foyer (REQ-NOT-005) : liste, ajout d'un **webhook** (URL) et suppression.
 * L'URL est validée côté serveur contre la falsification de requête (SSRF) — une adresse interne/bouclage
 * est refusée (422), signalée ici. Isolée par le serveur (§9). Aucune chaîne d'affichage en dur
 * (REQ-I18N-002) ; s'appuie sur le client généré.
 *
 * @implements REQ-NOT-005
 */
export function NotificationChannelsCard() {
  const { t } = useTranslation();
  const [channels, setChannels] = useState<NotificationChannelDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [rejected, setRejected] = useState(false);
  const [url, setUrl] = useState("");

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

  async function addWebhook() {
    const { response } = await api.POST("/notifications/channels", {
      body: { kind: "webhook", config: { url } },
    });
    if (!response.ok) {
      // URL refusée (SSRF) ou invalide : signalé, la liste reste inchangée.
      setRejected(true);
      return;
    }
    setRejected(false);
    setUrl("");
    await refresh();
  }

  async function remove(id: string) {
    await api.DELETE("/notifications/channels/{id}", { params: { path: { id } } });
    await refresh();
  }

  /** Résout l'URL d'un webhook depuis la config générique (objet ouvert). */
  function webhookUrl(channel: NotificationChannelDto): string {
    const config = channel.config as { url?: unknown };
    return typeof config?.url === "string" ? config.url : "";
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
          {t("notificationChannels.urlRejected")}
        </p>
      )}

      <div>
        <input
          data-testid="notification-channel-url"
          aria-label={t("notificationChannels.urlLabel")}
          placeholder={t("notificationChannels.urlPlaceholder")}
          value={url}
          onChange={(e) => setUrl(e.target.value)}
        />
        <button
          type="button"
          data-testid="notification-channel-add"
          onClick={() => void addWebhook()}
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
              <span data-testid="notification-channel-target">{webhookUrl(channel)}</span>
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
