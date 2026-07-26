import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type DeviceSummary = components["schemas"]["DeviceSummary"];

/**
 * Liste des appareils appairés du foyer, avec révocation individuelle et distinction de l'appareil
 * courant. S'appuie exclusivement sur le client généré (openapi-fetch) ; aucun type d'entité écrit
 * à la main, aucune chaîne littérale en JSX (REQ-I18N-002).
 *
 * @implements REQ-AUT-006
 */
export function DevicesList() {
  const { t } = useTranslation();
  const [devices, setDevices] = useState<DeviceSummary[]>([]);
  const [failed, setFailed] = useState(false);

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/devices");
    if (response.ok && data) {
      setDevices(data);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function revoke(id: string) {
    await api.DELETE("/devices/{id}", { params: { path: { id } } });
    await refresh();
  }

  return (
    <section data-testid="devices-list" aria-label={t("devices.title")}>
      <h2>{t("devices.title")}</h2>

      {failed && (
        <p data-testid="devices-error" role="alert">
          {t("devices.loadError")}
        </p>
      )}

      {!failed && devices.length === 0 && (
        <p data-testid="devices-empty">{t("devices.empty")}</p>
      )}

      <ul>
        {devices.map((device) => (
          <li key={device.id} data-testid="device-row">
            <span data-testid="device-label">{device.label}</span>
            <span data-testid="device-platform">
              {t("devices.platform")}: {device.platform}
            </span>
            <span data-testid="device-last-seen">
              {t("devices.lastSeen")}: {device.last_seen_at}
            </span>
            {device.current && (
              <span data-testid="device-current" role="status">
                {t("devices.current")}
              </span>
            )}
            <button
              type="button"
              data-testid={`device-revoke-${device.id}`}
              onClick={() => void revoke(device.id)}
            >
              {t("devices.revoke")}
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
