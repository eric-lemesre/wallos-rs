import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type PayerDto = components["schemas"]["PayerDto"];

/**
 * Gestion des payeurs du foyer (REQ-SUB-017) : liste, création, renommage, suppression. Un payeur est
 * une étiquette nominative (pas de compte). Isolée par le serveur (§9) ; un payeur créé apparaît
 * immédiatement (source du formulaire d'abonnement). La suppression d'un payeur référencé est refusée
 * par le serveur (409). Aucune chaîne d'affichage en dur (REQ-I18N-002). Calque des moyens de paiement.
 *
 * @implements REQ-SUB-017
 */
export function PayersList() {
  const { t } = useTranslation();
  const [payers, setPayers] = useState<PayerDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [inUse, setInUse] = useState(false);
  const [newName, setNewName] = useState("");

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/payers");
    if (response.ok && data) {
      setPayers(data);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function create() {
    await api.POST("/payers", { body: { name: newName } });
    setNewName("");
    await refresh();
  }

  async function rename(id: string, name: string) {
    await api.PUT("/payers/{id}", { params: { path: { id } }, body: { name } });
    await refresh();
  }

  async function remove(id: string) {
    const { response } = await api.DELETE("/payers/{id}", { params: { path: { id } } });
    if (!response.ok) {
      // Suppression refusée (payeur référencé, 409) : signalé, la liste reste inchangée.
      setInUse(true);
      return;
    }
    setInUse(false);
    await refresh();
  }

  return (
    <section data-testid="payers-list" aria-label={t("payers.title")}>
      <h2>{t("payers.title")}</h2>

      {failed && (
        <p data-testid="payers-error" role="alert">
          {t("payers.loadError")}
        </p>
      )}

      {inUse && (
        <p data-testid="payer-delete-error" role="alert">
          {t("payers.inUse")}
        </p>
      )}

      <div>
        <input
          data-testid="payer-new-name"
          aria-label={t("payers.name")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <button type="button" data-testid="payer-create" onClick={() => void create()}>
          {t("payers.create")}
        </button>
      </div>

      <ul>
        {payers.map((payer) => (
          <li key={payer.id} data-testid="payer-row">
            <span data-testid="payer-name">{payer.name}</span>
            <PayerRow
              payer={payer}
              onRename={rename}
              onDelete={remove}
              renameLabel={t("payers.rename")}
              deleteLabel={t("payers.delete")}
              editLabel={t("payers.name")}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

function PayerRow({
  payer,
  onRename,
  onDelete,
  renameLabel,
  deleteLabel,
  editLabel,
}: {
  payer: PayerDto;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  renameLabel: string;
  deleteLabel: string;
  editLabel: string;
}) {
  const [draft, setDraft] = useState(payer.name);
  return (
    <>
      <input
        data-testid={`payer-edit-${payer.id}`}
        aria-label={editLabel}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
      />
      <button
        type="button"
        data-testid={`payer-rename-${payer.id}`}
        onClick={() => void onRename(payer.id, draft)}
      >
        {renameLabel}
      </button>
      <button
        type="button"
        data-testid={`payer-delete-${payer.id}`}
        onClick={() => void onDelete(payer.id)}
      >
        {deleteLabel}
      </button>
    </>
  );
}
