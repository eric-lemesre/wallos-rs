import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import { clearOutbox, enqueue, flushOutbox, loadOutbox } from "./outbox";

/** @implements REQ-SYN-007 */

afterEach(() => {
  clearOutbox();
  vi.restoreAllMocks();
});

describe("outbox hors ligne", () => {
  it("empile les opérations et les conserve", () => {
    enqueue({ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } });
    enqueue({ op: "delete", entity_type: "payer", id: "b" });
    expect(loadOutbox()).toHaveLength(2);
    expect(loadOutbox()[0]).toMatchObject({ op: "upsert", id: "a" });
  });

  it("pousse la file et la vide quand l'envoi aboutit", async () => {
    enqueue({ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } });
    const post = vi.spyOn(api, "POST").mockResolvedValue({
      data: { results: [] },
      response: new Response(null, { status: 200 }),
    } as never);

    expect(await flushOutbox()).toBe(true);
    expect(post).toHaveBeenCalledWith("/sync/push", {
      body: { operations: [{ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } }] },
    });
    expect(loadOutbox()).toHaveLength(0);
  });

  it("conserve la file si l'envoi échoue (nouvel essai possible)", async () => {
    enqueue({ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } });
    vi.spyOn(api, "POST").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);

    expect(await flushOutbox()).toBe(false);
    expect(loadOutbox()).toHaveLength(1);
  });

  it("une file vide est considérée comme drainée sans appel réseau", async () => {
    const post = vi.spyOn(api, "POST");
    expect(await flushOutbox()).toBe(true);
    expect(post).not.toHaveBeenCalled();
  });
});
