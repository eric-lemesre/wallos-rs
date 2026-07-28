import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-CUR-001 — devise de référence. La persistance par foyer (retrouvée après rechargement / autre
// modalité) et l'isolation sont spécifiques à subtrack -> @design.
test.describe("Devise de référence", { tag: ["@design", "@REQ-CUR-001"] }, () => {
  test("le choix de devise de référence persiste après rechargement", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `ref-cur-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    // Le réglage se charge avec la session : recharge après connexion (comme les autres vues).
    await page.reload();

    // Défaut EUR, puis passage en USD.
    expect(await app.readReferenceCurrency()).toContain("EUR");
    await app.setReferenceCurrency("USD");
    expect(await app.readReferenceCurrency()).toContain("USD");

    // Le choix persiste après rechargement (réglage porté par le foyer, pas le navigateur).
    await page.reload();
    expect(await app.readReferenceCurrency()).toContain("USD");
  });
});
