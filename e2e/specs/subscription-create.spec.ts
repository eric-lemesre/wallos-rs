import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-002 — création d'abonnement. Le modèle (SUB-001) est capturé sur Wallos ; l'isolation et la
// validation par champ sont spécifiques à subtrack -> @design.
test.describe("Création d'abonnement", { tag: ["@design", "@REQ-SUB-002"] }, () => {
  test("crée un abonnement et affiche immédiatement la prochaine échéance", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    // Rattaché + prochaine échéance calculée immédiatement (first_payment futur -> lui-même).
    expect(await app.subscriptionNextPayment()).toContain("2030-01-31");
  });
});
