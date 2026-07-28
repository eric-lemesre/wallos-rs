import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-006 — liste et filtres. Le filtrage conjonctif côté serveur et le total converti reflétant
// le filtre sont des choix de conception subtrack (Wallos ne convertit pas) -> @design.
test.describe("Liste et filtres d'abonnements", { tag: ["@design", "@REQ-SUB-006"] }, () => {
  test("liste les abonnements avec un total, et le total reflète le filtre", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-list-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    await app.createSubscription({
      name: "Spotify", amount: "5.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });

    // La liste affiche les deux abonnements et un total qui les agrège (9.99 + 5.99 = 15.98 EUR).
    await app.refreshSubscriptions();
    expect(await app.subscriptionListed("Netflix")).toBe(true);
    expect(await app.subscriptionListed("Spotify")).toBe(true);
    expect(await app.subscriptionsTotal()).toContain("15.98");

    // Un filtre catégorie ne correspondant à aucun abonnement vide la liste : le total suit (nul).
    await app.filterSubscriptionsByCategory("99999999-9999-9999-9999-999999999999");
    expect(await app.subscriptionsEmpty()).toBe(true);
    expect(await app.subscriptionsTotal()).not.toContain("15.98");
  });
});
