import { useTranslation } from "react-i18next";

import { useOfflineSync } from "../sync/useOfflineSync";

/**
 * Indicateur de l'état de synchronisation (REQ-SYN-007) : signale que le client est **hors ligne** ou
 * qu'il reste des modifications **non synchronisées**, et confirme la synchronisation une fois le réseau
 * revenu (automatiquement, sans action de l'utilisateur). Aucune chaîne d'affichage en dur (REQ-I18N-002).
 *
 * @implements REQ-SYN-007
 */
export function SyncStatus() {
  const { t } = useTranslation();
  const { status, pendingCount } = useOfflineSync();

  return (
    <div data-testid="sync-status" data-status={status} role="status">
      {status === "offline" && <span>{t("sync.offline")}</span>}
      {status === "pending" && <span>{t("sync.pending", { count: pendingCount })}</span>}
      {status === "synced" && <span>{t("sync.synced")}</span>}
    </div>
  );
}
