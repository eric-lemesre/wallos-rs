import { api } from "../api/client";
import type { components } from "../api/client";

type SyncOperationInput = components["schemas"]["SyncOperationInput"];

/**
 * File d'attente locale des écritures effectuées **hors ligne** (REQ-SYN-007). Chaque opération est
 * conservée dans `localStorage` jusqu'à ce que la connectivité revienne, puis poussée au serveur via
 * `POST /sync/push` (REQ-SYN-004). Aucune dépendance externe, aucun service worker : un simple relais
 * durable cohérent avec la modalité web responsive (OQ-009, pas de coquille native).
 *
 * @implements REQ-SYN-007
 */
export type PendingOp = {
  op: "upsert" | "delete";
  entity_type: string;
  id: string;
  payload?: Record<string, unknown>;
};

const KEY = "wallos.outbox";

/** Événement (fenêtre) émis quand la file change (ajout) — l'indicateur de statut s'y abonne. */
export const QUEUED_EVENT = "wallos:queued";
/** Événement émis après une synchronisation réussie (file vidée) — les vues rechargent. */
export const SYNCED_EVENT = "wallos:synced";

/** Opérations en attente (jamais d'exception : un stockage illisible est traité comme vide). */
export function loadOutbox(): PendingOp[] {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as PendingOp[]) : [];
  } catch {
    return [];
  }
}

function save(ops: PendingOp[]): void {
  localStorage.setItem(KEY, JSON.stringify(ops));
}

/** Ajoute une opération à la file et signale le changement. */
export function enqueue(op: PendingOp): void {
  const ops = loadOutbox();
  ops.push(op);
  save(ops);
  window.dispatchEvent(new Event(QUEUED_EVENT));
}

/** Vide la file. */
export function clearOutbox(): void {
  localStorage.removeItem(KEY);
}

/** État de connectivité du navigateur (REQ-SYN-007). En environnement sans `navigator`, suppose en ligne. */
export function isOnline(): boolean {
  return typeof navigator === "undefined" ? true : navigator.onLine;
}

/**
 * Pousse la file au serveur (REQ-SYN-004) et la vide si la requête aboutit. Renvoie `true` si la file
 * est (ou était déjà) drainée, `false` si l'envoi a échoué (elle est conservée pour un nouvel essai).
 *
 * Une fois le lot **accepté** (HTTP 2xx), la file est vidée quel que soit le statut par opération : les
 * rejets éventuels (conflits) sont enregistrés côté serveur (journal REQ-SYN-005) et ne doivent pas
 * bloquer indéfiniment la synchronisation.
 */
export async function flushOutbox(): Promise<boolean> {
  const ops = loadOutbox();
  if (ops.length === 0) {
    return true;
  }
  // Le schéma OpenAPI type le `payload` comme objet libre (rendu `Record<string, never>` par
  // openapi-typescript) ; nos charges utiles arbitraires y sont adaptées à l'envoi.
  const operations = ops.map(
    (o) => ({ ...o, payload: o.payload }) as unknown as SyncOperationInput,
  );
  try {
    const { response } = await api.POST("/sync/push", { body: { operations } });
    if (!response.ok) {
      return false;
    }
  } catch {
    return false;
  }
  clearOutbox();
  window.dispatchEvent(new Event(SYNCED_EVENT));
  return true;
}
