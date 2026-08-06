import { api } from "../api/client";
import type { components } from "../api/client";

/**
 * Synchronisation initiale d'un appareil nouvellement **appairé** (REQ-SYN-008). Un appareil appairé
 * (jeton d'appareil, REQ-AUT-005) récupère **l'intégralité** des données du compte via le delta
 * incrémental (`GET /sync/changes`, REQ-SYN-003), **paginé** et **reprenable** : après chaque lot
 * appliqué, le curseur est **persisté** ; si le processus est interrompu, un nouvel appel **reprend du
 * dernier lot appliqué**, sans repartir de zéro.
 *
 * @implements REQ-SYN-008
 */
export type SyncChange = components["schemas"]["SyncChangeDto"];

const CURSOR_KEY = "wallos.initialSync.cursor";
const DONE_KEY = "wallos.initialSync.done";

/** Curseur de reprise persisté (ou `null` si la synchronisation initiale n'a jamais démarré). */
export function initialSyncCursor(): string | null {
  return localStorage.getItem(CURSOR_KEY);
}

/** Vrai si la synchronisation initiale s'est achevée. */
export function initialSyncDone(): boolean {
  return localStorage.getItem(DONE_KEY) === "1";
}

/** Réinitialise l'état (nouvel appairage) : la prochaine synchronisation repart de l'origine. */
export function resetInitialSync(): void {
  localStorage.removeItem(CURSOR_KEY);
  localStorage.removeItem(DONE_KEY);
}

/**
 * Draine l'intégralité du delta depuis le curseur persisté (ou l'origine), en appelant `applyBatch`
 * pour chaque page **puis** en persistant le curseur — d'où la reprenabilité. Une exception d'`applyBatch`
 * (ou du réseau) interrompt le drain **après** le dernier curseur persisté : un nouvel appel reprend
 * exactement là. `pageSize` borne la taille de page (le serveur borne aussi côté API).
 *
 * @implements REQ-SYN-008
 */
export async function runInitialSync(
  applyBatch: (changes: SyncChange[]) => void,
  pageSize = 100,
): Promise<void> {
  let cursor = initialSyncCursor() ?? undefined;
  for (;;) {
    const { data, response } = await api.GET("/sync/changes", {
      params: { query: { cursor, limit: pageSize } },
    });
    if (!response.ok || !data) {
      throw new Error("initial sync request failed");
    }
    // Appliquer AVANT de persister le curseur : si l'application échoue, le curseur reste sur le lot
    // précédent et le lot en cours sera rejoué (au pire une fois — les upserts sont idempotents).
    applyBatch(data.changes);
    cursor = data.next_cursor;
    localStorage.setItem(CURSOR_KEY, cursor);
    if (!data.has_more) {
      break;
    }
  }
  localStorage.setItem(DONE_KEY, "1");
}
