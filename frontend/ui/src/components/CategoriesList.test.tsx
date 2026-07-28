import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { I18nextProvider } from "react-i18next";
import { afterEach, describe, expect, it, vi } from "vitest";

import { api } from "../api/client";
import i18n from "../i18n";
import { CategoriesList } from "./CategoriesList";

/** @implements REQ-CAT-001 */

const CAT = { id: "11111111-1111-1111-1111-111111111111", name: "Streaming" };

function ok(data: unknown) {
  return { data, response: new Response(null, { status: 200 }) } as never;
}

function renderList() {
  return render(
    <I18nextProvider i18n={i18n}>
      <CategoriesList />
    </I18nextProvider>,
  );
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("CategoriesList", () => {
  it("liste les catégories", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([CAT]));
    renderList();
    expect(await screen.findByTestId("category-name")).toHaveTextContent("Streaming");
  });

  it("préserve l'ordre déterministe renvoyé par le serveur (REQ-CAT-005)", async () => {
    // Le serveur renvoie un ordre déterministe (par nom) ; l'UI l'affiche tel quel, identique aux
    // autres modalités.
    const ordered = [
      { id: "a", name: "Alpha" },
      { id: "b", name: "Beta" },
      { id: "c", name: "Gamma" },
    ];
    vi.spyOn(api, "GET").mockResolvedValue(ok(ordered));
    renderList();
    await screen.findByText("Alpha");
    const names = screen.getAllByTestId("category-name").map((n) => n.textContent);
    expect(names).toEqual(["Alpha", "Beta", "Gamma"]);
  });

  it("crée une catégorie puis rafraîchit", async () => {
    const get = vi.spyOn(api, "GET").mockResolvedValueOnce(ok([])).mockResolvedValueOnce(ok([CAT]));
    const post = vi.spyOn(api, "POST").mockResolvedValue(ok(CAT));
    const user = userEvent.setup();
    renderList();

    await user.type(screen.getByTestId("category-new-name"), "Streaming");
    await user.click(screen.getByTestId("category-create"));
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/categories", { body: { name: "Streaming" } }),
    );
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(await screen.findByTestId("category-name")).toHaveTextContent("Streaming");
  });

  it("renomme une catégorie (PUT avec paramètre de chemin)", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([CAT]));
    const put = vi.spyOn(api, "PUT").mockResolvedValue(ok({ ...CAT, name: "Musique" }));
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("category-name");
    const edit = screen.getByTestId(`category-edit-${CAT.id}`);
    await user.clear(edit);
    await user.type(edit, "Musique");
    await user.click(screen.getByTestId(`category-rename-${CAT.id}`));
    await waitFor(() =>
      expect(put).toHaveBeenCalledWith("/categories/{id}", {
        params: { path: { id: CAT.id } },
        body: { name: "Musique" },
      }),
    );
  });

  it("supprime une catégorie", async () => {
    vi.spyOn(api, "GET").mockResolvedValue(ok([CAT]));
    const del = vi.spyOn(api, "DELETE").mockResolvedValue(ok(undefined));
    const user = userEvent.setup();
    renderList();

    await screen.findByTestId("category-name");
    await user.click(screen.getByTestId(`category-delete-${CAT.id}`));
    await waitFor(() =>
      expect(del).toHaveBeenCalledWith("/categories/{id}", {
        params: { path: { id: CAT.id } },
      }),
    );
  });
});
