import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

/**
 * Réplique dans le document de **production** la politique de sécurité de contenu délivrée par le
 * serveur en en-tête HTTP (REQ-SEC-006). `script-src 'self'` : aucune directive permissive de script
 * en ligne (critère #1). Appliqué **au build uniquement** (`apply: "build"`) : en dev/e2e, Vite sert
 * l'index sans CSP pour ne pas bloquer le client HMR (scripts en ligne, eval, WebSocket).
 *
 * @implements REQ-SEC-006
 */
const CONTENT_SECURITY_POLICY =
  "default-src 'self'; base-uri 'self'; object-src 'none'; frame-ancestors 'none'; " +
  "form-action 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; " +
  "img-src 'self' data:; font-src 'self'; connect-src 'self'";

function contentSecurityPolicy(): Plugin {
  return {
    name: "wallos-csp-meta",
    apply: "build",
    transformIndexHtml(html) {
      return {
        html,
        tags: [
          {
            tag: "meta",
            attrs: {
              "http-equiv": "Content-Security-Policy",
              content: CONTENT_SECURITY_POLICY,
            },
            injectTo: "head-prepend",
          },
        ],
      };
    },
  };
}

// Coquille web servie en dev/e2e. Le proxy renvoie /api vers le serveur Rust (REQ-AUT-001).
export default defineConfig({
  plugins: [react(), contentSecurityPolicy()],
  server: {
    port: 5173,
    // Autorise l'import de l'UI partagée située hors de ce dossier (../../ui/src).
    fs: { allow: [".."] },
    proxy: {
      "/api": {
        target: "http://localhost:3000",
        changeOrigin: true,
      },
    },
  },
});
