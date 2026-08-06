import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-NOT-001 — Rappel avant échéance. La RÈGLE de déclenchement (exact, à N jours) et le REGROUPEMENT
// par compte sont capturés sur Wallos (cron sendnotifications.php) et asserés en intégration ; l'émission
// réelle passe par un cron. Cet e2e vérifie le parcours UI : un abonnement échéant demain apparaît dans
// la vue des rappels du jour (délai par défaut 1). Vue propre à subtrack -> @design.
test.describe("Rappels avant échéance", { tag: ["@design", "@REQ-NOT-001"] }, () => {
  test("un abonnement échéant demain apparaît dans les rappels du jour", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `not001-${stamp}@example.com`;
    const name = `Netflix ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Échéance demain (délai de rappel par défaut = 1 jour) : un rappel est dû aujourd'hui.
    const tomorrow = new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
    await app.createSubscription({
      name, amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: tomorrow,
    });

    // Recharge pour que la carte des rappels interroge le serveur.
    await page.reload();
    await expect.poll(() => app.reminderNames()).toContain(name);
  });
});
