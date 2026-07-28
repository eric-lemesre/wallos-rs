import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-011 — moyens de paiement. `oracle: legacy` mais l'ISOLATION (§9) est spécifique à subtrack,
// non reproductible sur Wallos mono-foyer -> @design (calque de la gestion des catégories).
test.describe("Moyens de paiement", { tag: ["@design", "@REQ-SUB-011"] }, () => {
  test("un moyen de paiement créé est visible immédiatement et isolé des autres comptes", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const alice = `alice-pm-${stamp}@example.com`;
    const bob = `bob-pm-${stamp}@example.com`;
    const name = `Carte Alice ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email: alice, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: alice, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await app.createPaymentMethod(name);
    expect(await app.paymentMethodVisible(name)).toBe(true);

    // Bob (autre compte) ne voit jamais le moyen de paiement d'Alice.
    await app.gotoSignup();
    await app.signup({ email: bob, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: bob, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await page.reload();
    expect(await app.paymentMethodAbsent(name)).toBe(true);
  });
});
