import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@wallos/ui";
// Feuille unique du design system, importée ICI et nulle part ailleurs : un composant qui
// l'importerait ferait échouer `tsc --noEmit` (REQ-CLT-008, ADR 0057).
import "@wallos/ui/src/ds/wallos-ux.css";

// Coquille web : elle monte la racine partagée, rien d'autre (AGENTS.md §7, ADR 0057).
// L'API est servie sur la même origine (REQ-OPS-003) — aucune origine à fournir.
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <App canal="web" />
    </StrictMode>,
  );
}
