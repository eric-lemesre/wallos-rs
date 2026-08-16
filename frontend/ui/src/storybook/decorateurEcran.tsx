import type { Decorator } from "@storybook/react-vite";
import { I18nextProvider } from "react-i18next";

import i18n from "../i18n";

/**
 * Décorateur « coquille » : place la story dans le même contexte que l'application réelle
 * (ADR 0058). Aujourd'hui l'i18n seule ; l'authentification et le routage s'y ajouteront quand des
 * écrans en dépendront, au même endroit — plutôt que répétés dans chaque story.
 */
export const decorateurEcran: Decorator = (Story) => (
  <I18nextProvider i18n={i18n}>
    <main>
      <Story />
    </main>
  </I18nextProvider>
);
