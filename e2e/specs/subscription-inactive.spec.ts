import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-008 — abonnement désactivé exclu des agrégats. L'exclusion du total et la conservation dans la
// liste sont observables ; le volet « aucun rappel » relève de l'ordonnanceur NOT (hors périmètre) -> @design.
test.describe("Abonnement désactivé", { tag: ["@design", "@REQ-SUB-008"] }, () => {
  test("un abonnement désactivé est conservé mais exclu du total", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-inactive-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    // Poll AVEC re-rafraîchissement : le premier refresh peut précéder le commit de la création.
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionsTotal();
      })
      .toContain("9.99");

    // Désactivation : l'abonnement reste listé, mais le total tombe (exclu de l'agrégat).
    await app.deactivateSubscription("Netflix");
    expect(await app.subscriptionListed("Netflix")).toBe(true);
    // Même barrière côté décroissance : la désactivation doit être committée ET relue.
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionsTotal();
      })
      .not.toContain("9.99");
  });
});
