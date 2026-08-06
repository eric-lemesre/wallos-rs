import { useCallback, useEffect, useState } from "react";

import { QUEUED_EVENT, SYNCED_EVENT, flushOutbox, isOnline, loadOutbox } from "./outbox";

/** État de synchronisation présenté à l'utilisateur (REQ-SYN-007). */
export type SyncState = "synced" | "offline" | "pending";

/**
 * Suit l'état de connectivité et la file d'attente hors ligne (REQ-SYN-007), et **déclenche
 * automatiquement** la synchronisation au retour du réseau — l'utilisateur n'a aucune action à
 * effectuer. Renvoie l'état affichable et le nombre d'opérations en attente.
 *
 * @implements REQ-SYN-007
 */
export function useOfflineSync(): { status: SyncState; pendingCount: number } {
  const [online, setOnline] = useState(isOnline());
  const [pendingCount, setPendingCount] = useState(loadOutbox().length);

  const refreshCount = useCallback(() => setPendingCount(loadOutbox().length), []);

  useEffect(() => {
    function onOnline() {
      setOnline(true);
      // Synchronisation automatique au retour de la connectivité, sans action de l'utilisateur.
      void flushOutbox().then(refreshCount);
    }
    function onOffline() {
      setOnline(false);
    }
    window.addEventListener("online", onOnline);
    window.addEventListener("offline", onOffline);
    window.addEventListener(QUEUED_EVENT, refreshCount);
    window.addEventListener(SYNCED_EVENT, refreshCount);
    // Tentative initiale : si des opérations sont en attente et qu'on est en ligne, on les pousse.
    if (isOnline()) {
      void flushOutbox().then(refreshCount);
    }
    return () => {
      window.removeEventListener("online", onOnline);
      window.removeEventListener("offline", onOffline);
      window.removeEventListener(QUEUED_EVENT, refreshCount);
      window.removeEventListener(SYNCED_EVENT, refreshCount);
    };
  }, [refreshCount]);

  const status: SyncState = !online ? "offline" : pendingCount > 0 ? "pending" : "synced";
  return { status, pendingCount };
}
