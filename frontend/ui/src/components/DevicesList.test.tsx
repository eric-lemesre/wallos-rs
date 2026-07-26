import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { DevicesList } from "./DevicesList";

/** @implements REQ-AUT-006 */

const LAPTOP = {
  id: "11111111-1111-1111-1111-111111111111",
  label: "Laptop",
  platform: "desktop",
  last_seen_at: "2026-07-26T10:00:00+00:00",
  current: true,
};
const PHONE = {
  id: "22222222-2222-2222-2222-222222222222",
  label: "Phone",
  platform: "mobile",
  last_seen_at: "2026-07-26T09:00:00+00:00",
  current: false,
};

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <DevicesList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("DevicesList", () => {
  it("liste les appareils avec libellé/plateforme et marque l'appareil courant", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([LAPTOP, PHONE]));
    renderList();

    expect(await screen.findAllByTestId("device-row")).toHaveLength(2);
    expect(screen.getAllByTestId("device-label").map((n) => n.textContent)).toEqual([
      "Laptop",
      "Phone",
    ]);
    // Un seul marqueur « appareil courant », pour le Laptop.
    const current = screen.getAllByTestId("device-current");
    expect(current).toHaveLength(1);
    expect(current[0]).toHaveTextContent(i18n.t("devices.current"));
  });

  it("affiche l'état vide quand aucun appareil n'est appairé", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([]));
    renderList();
    expect(await screen.findByTestId("devices-empty")).toHaveTextContent(
      i18n.t("devices.empty"),
    );
  });

  it("révoque un appareil (DELETE avec paramètre de chemin) puis rafraîchit", async () => {
    const get = vi
      .spyOn(api, "GET")
      .mockResolvedValueOnce(ok([LAPTOP, PHONE]))
      .mockResolvedValueOnce(ok([LAPTOP]));
    const del = vi
      .spyOn(api, "DELETE")
      .mockResolvedValue({ response: new Response(null, { status: 204 }) } as never);
    const user = userEvent.setup();
    renderList();

    await screen.findAllByTestId("device-row");
    await user.click(screen.getByTestId(`device-revoke-${PHONE.id}`));

    await waitFor(() =>
      expect(del).toHaveBeenCalledWith("/devices/{id}", {
        params: { path: { id: PHONE.id } },
      }),
    );
    // Rafraîchissement : deuxième GET, la liste ne contient plus que le Laptop.
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getAllByTestId("device-row")).toHaveLength(1));
  });

  it("signale une erreur de chargement", async () => {
    vi.spyOn(api, "GET").mockResolvedValue({
      data: undefined,
      response: new Response(null, { status: 500 }),
    } as never);
    renderList();
    expect(await screen.findByTestId("devices-error")).toHaveTextContent(
      i18n.t("devices.loadError"),
    );
  });
});
