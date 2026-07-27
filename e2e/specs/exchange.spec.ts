import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const STRONG_PASSWORD = "correct horse battery staple";

// REQ-CUR-004 — mode dégradé du fournisseur de taux (`oracle: design`, tag `@design`).
//
// Sans fournisseur configuré (base neuve : aucun taux étranger connu), une agrégation en devise
// étrangère doit rester fonctionnelle : le montant sans taux est EXCLU et l'agrégat est
// EXPLICITEMENT signalé incomplet — jamais présenté comme un zéro silencieux (critère #2). La part
// convertible (devise identique) est conservée et reste affichée.
test.describe("Agrégation multi-devises — mode dégradé", { tag: ["@design", "@REQ-CUR-004"] }, () => {
  test("signale un agrégat incomplet et n'affiche jamais un zéro silencieux", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-cur004-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: STRONG_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);

    await app.login({ email, password: STRONG_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Cible USD : 20 USD (identité, convertible) + 10 EUR (aucun taux connu → exclu).
    await app.computeAggregate("USD", [
      { amount: "20", currency: "USD" },
      { amount: "10", currency: "EUR" },
    ]);

    // L'agrégat est EXPLICITEMENT signalé incomplet (montant EUR exclu, sans taux).
    expect(await app.aggregateIncompleteVisible()).toBe(true);
    // La part convertible reste affichée : le total n'est pas silencieusement amputé/nul.
    const total = await app.readAggregateTotal();
    expect(total).toContain("20");
    expect(total).toContain("USD");
  });
});
