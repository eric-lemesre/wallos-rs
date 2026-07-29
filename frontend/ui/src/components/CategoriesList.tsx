import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type CategoryDto = components["schemas"]["CategoryDto"];

/**
 * Gestion des catégories du compte (REQ-CAT-001) : liste, création, renommage, suppression. Isolée
 * par le serveur (§9) ; une catégorie créée apparaît immédiatement dans la liste (source du
 * formulaire d'abonnement). S'appuie sur le client généré ; aucune chaîne d'affichage en dur (I18N-002).
 *
 * @implements REQ-CAT-001
 */
export function CategoriesList() {
  const { t } = useTranslation();
  const [categories, setCategories] = useState<CategoryDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [newName, setNewName] = useState("");
  // Refus de suppression d'une catégorie **référencée** par un abonnement (REQ-CAT-003, 409) : le
  // comportement de référence (capturé sur Wallos) est un blocage, jamais une réaffectation silencieuse.
  const [inUse, setInUse] = useState(false);

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/categories");
    if (response.ok && data) {
      setCategories(data);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function create() {
    await api.POST("/categories", { body: { name: newName } });
    setNewName("");
    await refresh();
  }

  async function rename(id: string, name: string) {
    await api.PUT("/categories/{id}", { params: { path: { id } }, body: { name } });
    await refresh();
  }

  async function remove(id: string) {
    const { response } = await api.DELETE("/categories/{id}", { params: { path: { id } } });
    // 409 : la catégorie est référencée par un abonnement -> suppression refusée (REQ-CAT-003).
    // On l'indique explicitement et on ne rafraîchit rien (la catégorie reste présente).
    if (response.status === 409) {
      setInUse(true);
      return;
    }
    setInUse(false);
    await refresh();
  }

  return (
    <section data-testid="categories-list" aria-label={t("categories.title")}>
      <h2>{t("categories.title")}</h2>

      {failed && (
        <p data-testid="categories-error" role="alert">
          {t("categories.loadError")}
        </p>
      )}

      {inUse && (
        <p data-testid="category-delete-error" role="alert">
          {t("categories.inUse")}
        </p>
      )}

      <div>
        <input
          data-testid="category-new-name"
          aria-label={t("categories.name")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <button type="button" data-testid="category-create" onClick={() => void create()}>
          {t("categories.create")}
        </button>
      </div>

      <ul>
        {categories.map((category) => (
          <li key={category.id} data-testid="category-row">
            <span data-testid="category-name">{category.name}</span>
            <CategoryRow
              category={category}
              onRename={rename}
              onDelete={remove}
              renameLabel={t("categories.rename")}
              deleteLabel={t("categories.delete")}
              editLabel={t("categories.name")}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

function CategoryRow({
  category,
  onRename,
  onDelete,
  renameLabel,
  deleteLabel,
  editLabel,
}: {
  category: CategoryDto;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  renameLabel: string;
  deleteLabel: string;
  editLabel: string;
}) {
  const [draft, setDraft] = useState(category.name);
  return (
    <>
      <input
        data-testid={`category-edit-${category.id}`}
        aria-label={editLabel}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
      />
      <button
        type="button"
        data-testid={`category-rename-${category.id}`}
        onClick={() => void onRename(category.id, draft)}
      >
        {renameLabel}
      </button>
      <button
        type="button"
        data-testid={`category-delete-${category.id}`}
        onClick={() => void onDelete(category.id)}
      >
        {deleteLabel}
      </button>
    </>
  );
}
