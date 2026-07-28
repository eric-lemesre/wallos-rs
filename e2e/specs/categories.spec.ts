import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-CAT-001 — gestion des catégories. `oracle: legacy` mais l'ISOLATION (crée/renomme/supprime
// n'affecte que ses propres catégories, §9) est spécifique à subtrack et non reproductible sur Wallos
// mono-foyer -> scénario @design (cf. e2e/fixtures/oracles/REQ-CAT-001-categories.json, OQ-007).
test.describe("Catégories", { tag: ["@design", "@REQ-CAT-001"] }, () => {
  test("une catégorie créée est visible immédiatement et isolée des autres comptes", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const alice = `alice-cat-${stamp}@example.com`;
    const bob = `bob-cat-${stamp}@example.com`;
    const catName = `Alice Streaming ${stamp}`;

    // Alice crée une catégorie ; elle apparaît immédiatement dans sa liste.
    await app.gotoSignup();
    await app.signup({ email: alice, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: alice, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await app.createCategory(catName);
    expect(await app.categoryVisible(catName)).toBe(true);

    // Bob (autre compte) ne voit jamais la catégorie d'Alice.
    await app.gotoSignup();
    await app.signup({ email: bob, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: bob, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    // Recharge : la liste des catégories est refetchée avec la session de Bob.
    await page.reload();
    expect(await app.categoryAbsent(catName)).toBe(true);
  });
});
