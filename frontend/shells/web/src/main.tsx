import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { I18nextProvider } from "react-i18next";

import { SignupForm } from "../../../ui/src/components/SignupForm";
import i18n from "../../../ui/src/i18n";

// Coquille web : mince par conception (AGENTS.md §7). Elle se contente de monter l'UI partagée.
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nextProvider i18n={i18n}>
        <SignupForm />
      </I18nextProvider>
    </StrictMode>,
  );
}
