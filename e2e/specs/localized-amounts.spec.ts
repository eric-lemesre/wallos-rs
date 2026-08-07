import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-CUR-006 — Formatage localisé des montants (oracle design). Séparateurs et position du
// symbole suivent la LOCALE (dérivée de la langue de l'interface, ADR 0051), le nombre de
// décimales suit la DEVISE (Intl/CLDR). La valeur, elle, ne change jamais.
test.describe("Formatage localisé des montants", { tag: ["@design", "@REQ-CUR-006"] }, () => {
  test("le même montant s'affiche en conventions anglaises puis françaises", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `cur006-${stamp}@example.com`;
    const name = `Netflix ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await page.reload();

    const future = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
    await app.createSubscription({
      name, amount: "1234.5", currency: "EUR", unit: "month", interval: "1", firstPayment: future,
    });
    await page.reload();

    // Anglais (défaut du navigateur de test) : symbole avant, point décimal, virgule de milliers.
    await app.setLanguage("en");
    await expect
      .poll(async () => await app.subscriptionAmount(name))
      .toContain("€1,234.50");

    // Français : symbole après, virgule décimale, espace de milliers — même valeur.
    await app.setLanguage("fr");
    await expect.poll(async () => await app.subscriptionAmount(name)).toMatch(/1\s234,50\s*€/u);
  });
});
