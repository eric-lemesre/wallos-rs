import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Coquille web servie en dev/e2e. Le proxy renvoie /api vers le serveur Rust (REQ-AUT-001).
export default defineConfig({
  plugins: [react()],
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
