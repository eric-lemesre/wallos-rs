import type { StorybookConfig } from "@storybook/react-vite";

/**
 * Atelier de composants (ADR 0058). Réemploie le Vite du paquet plutôt qu'une seconde chaîne de
 * construction : ce qui est rendu ici est bâti comme ce qui est livré.
 */
const config: StorybookConfig = {
  stories: ["../src/**/*.stories.@(ts|tsx)"],
  addons: ["@storybook/addon-docs", "msw-storybook-addon"],
  framework: { name: "@storybook/react-vite", options: {} },
  // Chemin relatif à `.storybook/`, pas à la racine du paquet : sert le service worker de MSW.
  staticDirs: ["../public"],
  core: { disableTelemetry: true },
};

export default config;
