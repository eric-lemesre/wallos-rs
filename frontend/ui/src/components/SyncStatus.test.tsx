import { render, screen, waitFor } from "@testing-library/react";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { clearOutbox, enqueue } from "../sync/outbox";
import { SyncStatus } from "./SyncStatus";

/** @implements REQ-SYN-007 */

function renderStatus() {
  return render(
    <I18nextProvider i18n={i18n}>
      <SyncStatus />
    </I18nextProvider>,
  );
}

afterEach(() => {
  clearOutbox();
  vi.restoreAllMocks();
  // Rétablit l'état en ligne par défaut.
  Object.defineProperty(navigator, "onLine", { value: true, configurable: true });
});

describe("SyncStatus", () => {
  it("affiche « synchronisé » en ligne sans opération en attente", () => {
    renderStatus();
    expect(screen.getByTestId("sync-status")).toHaveAttribute("data-status", "synced");
  });

  it("affiche « hors ligne » quand la connectivité est absente", () => {
    Object.defineProperty(navigator, "onLine", { value: false, configurable: true });
    renderStatus();
    window.dispatchEvent(new Event("offline"));
    expect(screen.getByTestId("sync-status")).toHaveAttribute("data-status", "offline");
  });

  it("signale les opérations en attente hors ligne", () => {
    Object.defineProperty(navigator, "onLine", { value: false, configurable: true });
    renderStatus();
    window.dispatchEvent(new Event("offline"));
    enqueue({ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } });
    // Hors ligne, l'attente est visible ; le statut « offline » prime tant que le réseau est absent.
    expect(screen.getByTestId("sync-status")).toBeInTheDocument();
  });

  it("pousse automatiquement la file au retour de la connectivité", async () => {
    // Hors ligne : on empile une opération.
    Object.defineProperty(navigator, "onLine", { value: false, configurable: true });
    enqueue({ op: "upsert", entity_type: "payer", id: "a", payload: { name: "Alex" } });
    const post = vi.spyOn(api, "POST").mockResolvedValue({
      data: { results: [] },
      response: new Response(null, { status: 200 }),
    } as never);
    renderStatus();

    // Retour du réseau : synchronisation automatique, sans action de l'utilisateur.
    Object.defineProperty(navigator, "onLine", { value: true, configurable: true });
    window.dispatchEvent(new Event("online"));
    await waitFor(() => expect(post).toHaveBeenCalledWith("/sync/push", expect.anything()));
    await waitFor(() =>
      expect(screen.getByTestId("sync-status")).toHaveAttribute("data-status", "synced"),
    );
  });
});
