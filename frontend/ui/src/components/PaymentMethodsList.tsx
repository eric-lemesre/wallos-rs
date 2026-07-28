import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type PaymentMethodDto = components["schemas"]["PaymentMethodDto"];

/**
 * Gestion des moyens de paiement du compte (REQ-SUB-011) : liste, création, renommage, suppression.
 * Isolée par le serveur (§9) ; un moyen créé apparaît immédiatement dans la liste (source du formulaire
 * d'abonnement). S'appuie sur le client généré ; aucune chaîne d'affichage en dur (REQ-I18N-002).
 * Calque de la gestion des catégories.
 *
 * @implements REQ-SUB-011
 */
export function PaymentMethodsList() {
  const { t } = useTranslation();
  const [methods, setMethods] = useState<PaymentMethodDto[]>([]);
  const [failed, setFailed] = useState(false);
  const [newName, setNewName] = useState("");

  const refresh = useCallback(async () => {
    const { data, response } = await api.GET("/payment-methods");
    if (response.ok && data) {
      setMethods(data);
      setFailed(false);
    } else {
      setFailed(true);
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function create() {
    await api.POST("/payment-methods", { body: { name: newName } });
    setNewName("");
    await refresh();
  }

  async function rename(id: string, name: string) {
    await api.PUT("/payment-methods/{id}", { params: { path: { id } }, body: { name } });
    await refresh();
  }

  async function remove(id: string) {
    await api.DELETE("/payment-methods/{id}", { params: { path: { id } } });
    await refresh();
  }

  return (
    <section data-testid="payment-methods-list" aria-label={t("paymentMethods.title")}>
      <h2>{t("paymentMethods.title")}</h2>

      {failed && (
        <p data-testid="payment-methods-error" role="alert">
          {t("paymentMethods.loadError")}
        </p>
      )}

      <div>
        <input
          data-testid="payment-method-new-name"
          aria-label={t("paymentMethods.name")}
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
        />
        <button type="button" data-testid="payment-method-create" onClick={() => void create()}>
          {t("paymentMethods.create")}
        </button>
      </div>

      <ul>
        {methods.map((method) => (
          <li key={method.id} data-testid="payment-method-row">
            <span data-testid="payment-method-name">{method.name}</span>
            <PaymentMethodRow
              method={method}
              onRename={rename}
              onDelete={remove}
              renameLabel={t("paymentMethods.rename")}
              deleteLabel={t("paymentMethods.delete")}
              editLabel={t("paymentMethods.name")}
            />
          </li>
        ))}
      </ul>
    </section>
  );
}

function PaymentMethodRow({
  method,
  onRename,
  onDelete,
  renameLabel,
  deleteLabel,
  editLabel,
}: {
  method: PaymentMethodDto;
  onRename: (id: string, name: string) => Promise<void>;
  onDelete: (id: string) => Promise<void>;
  renameLabel: string;
  deleteLabel: string;
  editLabel: string;
}) {
  const [draft, setDraft] = useState(method.name);
  return (
    <>
      <input
        data-testid={`payment-method-edit-${method.id}`}
        aria-label={editLabel}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
      />
      <button
        type="button"
        data-testid={`payment-method-rename-${method.id}`}
        onClick={() => void onRename(method.id, draft)}
      >
        {renameLabel}
      </button>
      <button
        type="button"
        data-testid={`payment-method-delete-${method.id}`}
        onClick={() => void onDelete(method.id)}
      >
        {deleteLabel}
      </button>
    </>
  );
}
