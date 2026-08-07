import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-009 — date de fin et annulation programmée. Le marquage « terminé » et l'exclusion du total
// sont des comportements observables ; l'ancrage des échéances (clamp) est un choix subtrack -> @design.
test.describe("Date de fin d'abonnement", { tag: ["@design", "@REQ-SUB-009"] }, () => {
  test("un abonnement dont la date de fin est dépassée est terminé et exclu du total", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-end-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Un abonnement en cours (fin lointaine) : compté dans le total.
    await app.createSubscription({
      name: "EnCours", amount: "10.00", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2030-01-15", endDate: "2999-12-31",
    });
    // Un abonnement dont la date de fin est déjà dépassée : terminé, exclu du total.
    await app.createSubscription({
      name: "Termine", amount: "5.00", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2020-01-15", endDate: "2020-12-31",
    });

    // Poll AVEC re-rafraîchissement jusqu'à présence des deux lignes (le premier refresh peut
    // précéder le commit de la dernière création — flake observé en suite parallèle webkit).
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionNames();
      })
      .toEqual(expect.arrayContaining(["EnCours", "Termine"]));
    expect(await app.subscriptionEnded("Termine")).toBe(true);
    // Le total ne compte que l'abonnement en cours (10.00), pas le terminé.
    expect(await app.subscriptionsTotal()).toContain("10.00");
  });
});
