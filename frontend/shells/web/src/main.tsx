import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { I18nextProvider } from "react-i18next";

import { ChangePasswordForm } from "../../../ui/src/components/ChangePasswordForm";
import { DevicesList } from "../../../ui/src/components/DevicesList";
import { LoginForm } from "../../../ui/src/components/LoginForm";
import { SignupForm } from "../../../ui/src/components/SignupForm";
import i18n from "../../../ui/src/i18n";

// Coquille web : mince par conception (AGENTS.md §7). Elle se contente de monter l'UI partagée.
// Montage minimal signup + login (routage TanStack différé jusqu'à une exigence de navigation).
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nextProvider i18n={i18n}>
        <main>
          <SignupForm />
          <LoginForm />
          <DevicesList />
          <ChangePasswordForm />
        </main>
      </I18nextProvider>
    </StrictMode>,
  );
}
