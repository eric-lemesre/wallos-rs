import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-017 — Suppression d'un payeur référencé. `oracle: legacy` : le comportement de référence
// (REFUS de suppression, jamais réaffectation ni cascade) est capturé sur Wallos 5.4.2 et gelé dans
// e2e/fixtures/oracles/REQ-SUB-017-payer.json (handleDeleteMember, `household_in_use`). L'ISOLATION du
// comptage par foyer (§9) étant spécifique à subtrack (Wallos est mono-foyer), ce scénario est tagué
// @design — comme le refus de suppression de catégorie (REQ-CAT-003).
test.describe("Suppression de payeur référencé", { tag: ["@design", "@REQ-SUB-017"] }, () => {
  test("un payeur utilisé par un abonnement ne peut pas être supprimé", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `payer-del-${stamp}@example.com`;
    const payerName = `Alex ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Crée le payeur, puis rattache un abonnement à ce payeur (via l'API, comme le ferait le formulaire).
    await app.createPayer(payerName);
    expect(await app.payerVisible(payerName)).toBe(true);
    await app.attachSubscriptionToPayer(`Netflix ${stamp}`, payerName);
    await page.reload();

    // Tente la suppression : refusée (payeur référencé), et le payeur reste présent.
    await app.deletePayer(payerName);
    expect(await app.payerDeleteRefused()).toBe(true);
    expect(await app.payerVisible(payerName)).toBe(true);
  });
});
