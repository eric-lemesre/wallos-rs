import { definePreview } from "@storybook/react-vite";
import addonMsw from "msw-storybook-addon";

// Feuille unique du design system, importée ICI — l'atelier est un point d'entrée au même titre
// qu'une coquille (REQ-CLT-008, ADR 0057). Aucun composant ne l'importe.
import "../src/ds/wallos-ux.css";

/**
 * API « CSF Next » plutôt que le chargeur historique, que la version installée déclare **déprécié**.
 * Adopter d'emblée l'interface courante évite une migration dès la première montée de version.
 */
export default definePreview({
  addons: [addonMsw()],
  parameters: {
    controls: { matchers: { color: /(background|color)$/i, date: /Date$/i } },
  },
});
