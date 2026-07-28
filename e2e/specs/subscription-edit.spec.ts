import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-004 — modification d'un abonnement. Le recalcul déterministe de l'échéance (ancrage+clamp) et
// l'isolation sont des choix subtrack -> @design. (La résolution de conflit concurrent relève de SYN-005.)
test.describe("Modification d'abonnement", { tag: ["@design", "@REQ-SUB-004"] }, () => {
  test("édite un abonnement en place et la liste reflète la modification", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-edit-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    await app.refreshSubscriptions();
    expect(await app.subscriptionAmount("Netflix")).toContain("9.99");

    // Modification en place : le nouveau montant est persisté et réaffiché après recalcul serveur.
    await app.editSubscriptionAmount("Netflix", "19.99");
    expect(await app.subscriptionAmount("Netflix")).toContain("19.99");
  });
});
