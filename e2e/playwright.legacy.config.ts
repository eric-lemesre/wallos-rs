import { defineConfig, devices } from "@playwright/test";

// Cible LEGACY (Wallos 5.4.2, ADR 0011) : configuration distincte de la cible `app`.
// `testDir: ./legacy` isole ces specs de `./specs` — la config app ne les exécute jamais, et
// réciproquement. Le conteneur Wallos est démarré par docker-compose (épinglé par digest).
export default defineConfig({
  testDir: "./legacy",
  timeout: 60_000,
  // Wallos (SQLite) est une instance unique partagée : exécution SÉRIE pour éviter les courses
  // (inscription concurrente du même utilisateur, verrous DB en first-run).
  workers: 1,
  fullyParallel: false,
  // Discipline anti-flakiness (§8.3) : 0 en local, 1 en CI.
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: "http://localhost:8282",
    trace: "on-first-retry",
  },
  // webkit OBLIGATOIRE (§8.2).
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: {
    command: "docker compose -f legacy/docker-compose.yml up",
    url: "http://localhost:8282/login.php",
    reuseExistingServer: !process.env.CI,
    timeout: 300_000,
  },
});
