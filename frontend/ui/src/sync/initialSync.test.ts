import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import { initialSyncCursor, initialSyncDone, resetInitialSync, runInitialSync } from "./initialSync";
import type { SyncChange } from "./initialSync";

/** @implements REQ-SYN-008 */

function page(changes: SyncChange[], nextCursor: string, hasMore: boolean) {
  return {
    data: { changes, next_cursor: nextCursor, has_more: hasMore, full_resync_required: true },
    response: new Response(null, { status: 200 }),
  } as never;
}

function change(id: string): SyncChange {
  return { kind: "upsert", entity_type: "payer", id };
}

afterEach(() => {
  resetInitialSync();
  vi.restoreAllMocks();
});

describe("synchronisation initiale (REQ-SYN-008)", () => {
  it("draine toutes les pages puis se marque terminée", async () => {
    vi.spyOn(api, "GET")
      .mockResolvedValueOnce(page([change("a"), change("b")], "cur-1", true))
      .mockResolvedValueOnce(page([change("c")], "cur-2", false));

    const applied: string[] = [];
    await runInitialSync((changes) => applied.push(...changes.map((c) => c.id)), 2);

    expect(applied).toEqual(["a", "b", "c"]);
    expect(initialSyncDone()).toBe(true);
  });

  it("reprend du dernier lot appliqué après une interruption, sans repartir de zéro", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValueOnce(page([change("a"), change("b")], "cur-1", true))
      // Deuxième page : disponible pour la reprise.
      .mockResolvedValueOnce(page([change("c")], "cur-2", false));

    const applied: string[] = [];
    // Première exécution : interrompue après le premier lot (applyBatch lève à la 2e invocation).
    let calls = 0;
    await expect(
      runInitialSync((changes) => {
        calls += 1;
        if (calls === 2) {
          throw new Error("interruption simulée");
        }
        applied.push(...changes.map((c) => c.id));
      }, 2),
    ).rejects.toThrow("interruption");

    // Le curseur du premier lot est persisté ; la synchro n'est pas marquée terminée.
    expect(initialSyncCursor()).toBe("cur-1");
    expect(initialSyncDone()).toBe(false);
    expect(applied).toEqual(["a", "b"]);

    // Reprise : repart du curseur persisté (cur-1) — la requête suivante le fournit, et seul le lot
    // restant est appliqué (pas de re-fetch depuis l'origine).
    get.mockReset();
    get.mockResolvedValueOnce(page([change("c")], "cur-2", false));
    await runInitialSync((changes) => applied.push(...changes.map((c) => c.id)), 2);

    // « a » et « b » ne sont pas ré-appliqués ; « c » complète la synchronisation.
    expect(applied).toEqual(["a", "b", "c"]);
    expect(get).toHaveBeenCalledWith("/sync/changes", {
      params: { query: { cursor: "cur-1", limit: 2 } },
    });
    expect(initialSyncDone()).toBe(true);
  });

  it("démarre à l'origine (curseur absent) au premier appairage", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValueOnce(page([change("a")], "cur-1", false));
    await runInitialSync(() => {}, 100);
    // Premier appel : aucun curseur fourni (synchronisation complète depuis l'origine).
    expect(get).toHaveBeenCalledWith("/sync/changes", {
      params: { query: { cursor: undefined, limit: 100 } },
    });
  });
});
