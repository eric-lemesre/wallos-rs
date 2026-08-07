import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-I18N-001 — choix et persistance de la langue. La persistance côté serveur (retrouvée après
// rechargement / autre modalité) est spécifique à subtrack -> @design.
test.describe("Langue", { tag: ["@design", "@REQ-I18N-001"] }, () => {
  test("le choix de langue persiste après rechargement", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `lang-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    // Le réglage se charge avec la session : recharge après connexion.
    await page.reload();

    // Choix du français, appliqué immédiatement.
    await app.setLanguage("fr");
    expect(await app.readLanguage()).toContain("fr");

    // Le choix persiste après rechargement (réglage porté par l'utilisateur, pas le navigateur).
    // Poll AVEC re-rechargement : si le GET /settings/language échoue transitoirement (CI chargée),
    // le composant replie sur la langue système — seule une nouvelle lecture serveur récupère.
    await expect
      .poll(async () => {
        await page.reload();
        return app.readLanguage();
      })
      .toContain("fr");
  });
});
