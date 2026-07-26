import { defineConfig, devices } from "@playwright/test";

// Cible legacy (Wallos 5.4.2, ADR 0011) non requise ici : REQ-AUT-001 est `oracle: design`.
const DATABASE_URL =
  process.env.DATABASE_URL ?? "postgres://postgres:postgres@localhost:5433/wallos";

export default defineConfig({
  testDir: "./specs",
  timeout: 30_000,
  // Discipline anti-flakiness (§8.3) : 0 en local, 1 en CI.
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: "http://localhost:5173",
    trace: "on-first-retry",
  },
  // Le projet webkit est OBLIGATOIRE (§8.2) : Chromium seul donne une fausse assurance sur macOS/Linux.
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "webkit", use: { ...devices["Desktop Safari"] } },
  ],
  webServer: [
    {
      command: "cargo run -p wallos-server",
      cwd: "..",
      env: { DATABASE_URL },
      url: "http://localhost:3000/api/v1/health",
      reuseExistingServer: !process.env.CI,
      timeout: 180_000,
    },
    {
      command: "npm run dev",
      cwd: "../frontend/shells/web",
      url: "http://localhost:5173",
      reuseExistingServer: !process.env.CI,
      timeout: 120_000,
    },
  ],
});
