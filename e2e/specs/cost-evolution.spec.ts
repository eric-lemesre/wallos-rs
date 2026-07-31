import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-STA-006 — évolution du coût mensuel sur 12 mois. Série historique (chaque point = abonnements
// actifs à ce mois-là) exprimée en devise de référence : conception subtrack -> @design.
test.describe("Évolution du coût sur 12 mois", { tag: ["@design", "@REQ-STA-006"] }, () => {
  test("affiche douze points, l'abonnement ancien pesant sur toute la fenêtre", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta006-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Abonnement démarré loin dans le passé : présent sur les douze points.
    await app.createSubscription({
      name: "Ancien", amount: "10.00", currency: "EUR", unit: "month", interval: "1", firstPayment: "2020-01-15",
    });

    await app.reloadCostEvolution();
    // La carte charge la série au montage puis au rechargement ; expect.poll absorbe le rendu asynchrone.
    await expect.poll(() => app.costEvolutionPointCount()).toBe(12);
  });
});
