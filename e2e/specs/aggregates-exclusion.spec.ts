import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-STA-003 — règle transverse d'exclusion des abonnements non actifs des agrégats. Le critère #2
// (réactivation -> réintégration immédiate) est directement observable sur le total de la liste : on
// désactive (le total tombe) puis on réactive (le total revient), sans autre action. @design (l'exclusion
// des états fin/essai étend la règle legacy `inactive = 0`).
test.describe("Exclusion transverse des agrégats", { tag: ["@design", "@REQ-STA-003"] }, () => {
  test("un abonnement réactivé est immédiatement réintégré au total", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta003-reactivate-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Spotify", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    await app.awaitSubscriptions(["Spotify"]);
    expect(await app.subscriptionsTotal()).toContain("9.99");

    // Désactivation : l'abonnement reste listé mais sort du total. Polls AVEC re-rafraîchissement :
    // la bascule doit être committée ET relue (OQ-012).
    await app.deactivateSubscription("Spotify");
    expect(await app.subscriptionListed("Spotify")).toBe(true);
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionsTotal();
      })
      .not.toContain("9.99");

    // Réactivation : réintégré immédiatement, le total le reflète de nouveau (critère #2).
    await app.reactivateSubscription("Spotify");
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionsTotal();
      })
      .toContain("9.99");
  });
});
