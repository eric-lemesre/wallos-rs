import { useState } from "react";
import { useTranslation } from "react-i18next";

import { api } from "../api/client";
import type { components } from "../api/client";

type DataBundle = components["schemas"]["DataBundle"];
type ImportReport = components["schemas"]["ImportReport"];

/**
 * Export et import des données du foyer (REQ-SUB-016). L'export récupère l'enveloppe complète et
 * l'expose (téléchargement + zone de texte lisible) ; l'import envoie une enveloppe et affiche le
 * **rapport** (créées + rejetées). Aucune chaîne d'affichage en dur (REQ-I18N-002) : tout passe par
 * le catalogue de traduction et le client typé généré.
 *
 * @implements REQ-SUB-016
 */
export function ImportExportCard() {
  const { t } = useTranslation();
  const [exported, setExported] = useState("");
  const [importText, setImportText] = useState("");
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function doExport() {
    setError(null);
    const { data, response } = await api.GET("/export");
    if (response.ok && data) {
      setExported(JSON.stringify(data, null, 2));
    } else {
      setError(t("importExport.exportError"));
    }
  }

  async function doImport() {
    setError(null);
    setReport(null);
    let parsed: DataBundle;
    try {
      parsed = JSON.parse(importText) as DataBundle;
    } catch {
      setError(t("importExport.invalidJson"));
      return;
    }
    const { data, response } = await api.POST("/import", { body: parsed });
    if (response.ok && data) {
      setReport(data);
    } else {
      setError(t("importExport.importError"));
    }
  }

  return (
    <section data-testid="import-export-card" aria-label={t("importExport.title")}>
      <h2>{t("importExport.title")}</h2>

      <button type="button" data-testid="export-button" onClick={() => void doExport()}>
        {t("importExport.export")}
      </button>
      {exported && (
        <textarea
          data-testid="export-output"
          readOnly
          rows={6}
          value={exported}
          aria-label={t("importExport.export")}
        />
      )}

      <label htmlFor="import-input">{t("importExport.importLabel")}</label>
      <textarea
        id="import-input"
        data-testid="import-input"
        rows={6}
        value={importText}
        onChange={(e) => setImportText(e.target.value)}
      />
      <button type="button" data-testid="import-button" onClick={() => void doImport()}>
        {t("importExport.import")}
      </button>

      {error && (
        <p data-testid="import-export-error" role="alert">
          {error}
        </p>
      )}

      {report && (
        <div data-testid="import-report">
          <p data-testid="import-created">
            {t("importExport.created", {
              categories: report.imported.categories,
              payment_methods: report.imported.payment_methods,
              subscriptions: report.imported.subscriptions,
            })}
          </p>
          <p data-testid="import-rejected-count">
            {t("importExport.rejectedCount", { count: report.rejected.length })}
          </p>
          <ul data-testid="import-rejected-list">
            {report.rejected.map((r) => (
              <li key={`${r.kind}:${r.reference}:${r.reason}`} data-testid="import-rejected">
                {r.kind} · {r.reference} · {r.reason}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
