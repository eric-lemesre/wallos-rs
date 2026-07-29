import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-CAT-003 — Suppression d'une catégorie référencée. `oracle: legacy` : le comportement de
// référence (REFUS de suppression, jamais réaffectation ni cascade) est capturé sur Wallos 5.4.2 et
// gelé dans e2e/fixtures/oracles/REQ-CAT-003-category-delete.json (handleDeleteCategory). L'ISOLATION
// du comptage par foyer (§9) étant spécifique à subtrack, ce scénario est tagué @design (non rejouable
// tel quel contre le legacy mono-foyer). Dette transverse §8.1-e2e (injection AppDriver) différée.
test.describe("Suppression de catégorie référencée", { tag: ["@design", "@REQ-CAT-003"] }, () => {
  test("une catégorie utilisée par un abonnement ne peut pas être supprimée", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `cat-del-${stamp}@example.com`;
    const catName = `Streaming ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Crée la catégorie, puis recharge pour que le formulaire d'abonnement peuple son sélecteur.
    await app.createCategory(catName);
    expect(await app.categoryVisible(catName)).toBe(true);
    await page.reload();

    // Crée un abonnement rattaché à cette catégorie.
    await app.createSubscription({
      name: `Netflix ${stamp}`,
      amount: "9.99",
      currency: "EUR",
      unit: "month",
      interval: "1",
      firstPayment: "2030-01-15",
      category: catName,
    });
    expect(await app.subscriptionNextPayment()).toContain("2030-01-15");

    // Tente la suppression : refusée (catégorie référencée), et la catégorie reste présente.
    await app.deleteCategory(catName);
    expect(await app.categoryDeleteRefused()).toBe(true);
    expect(await app.categoryVisible(catName)).toBe(true);
  });
});
