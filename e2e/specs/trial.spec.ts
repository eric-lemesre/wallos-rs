import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-010 — Période d'essai gratuit. Concept ABSENT de Wallos 5.4.2 (ADR 0041) : traité par
// conception -> @design. Un abonnement en essai est signalé et n'est pas compté dans le total tant que
// l'essai dure ; l'exclusion de coût est asserée en intégration, cet e2e vérifie le parcours UI.
test.describe("Période d'essai gratuit", { tag: ["@design", "@REQ-SUB-010"] }, () => {
  test("un abonnement en essai est signalé et exclu du total", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `sub010-${stamp}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Essai en cours (fin dans 30 jours) : l'abonnement est gratuit tant que l'essai dure.
    const trialEnd = new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
    await app.createSubscription({
      name: `EnEssai ${stamp}`, amount: "10.00", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2030-01-15", trialEnd,
    });

    // Barrière de persistance puis vérifie le badge d'essai sur la ligne (OQ-012).
    await app.awaitSubscriptions([`EnEssai ${stamp}`]);
    const row = page.getByTestId("subscription-row").filter({ hasText: `EnEssai ${stamp}` });
    await expect(row.getByTestId("subscription-trial")).toBeVisible();

    // Le total du foyer n'inclut pas l'abonnement en essai : son montant (10,00) est exclu.
    await expect(page.getByTestId("subscriptions-total")).not.toContainText("10.00");
  });
});
