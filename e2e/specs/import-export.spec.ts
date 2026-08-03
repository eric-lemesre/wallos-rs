import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-016 — import et export des données. Export réimportable + import tolérant (rapport de
// rejets). Format et sémantique de conception subtrack -> @design.
test.describe("Import et export des données", { tag: ["@design", "@REQ-SUB-016"] }, () => {
  test("exporte les données du foyer, puis importe une enveloppe avec un rejet", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub016-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Un abonnement à exporter.
    await app.createSubscription({
      name: "Netflix",
      amount: "9.99",
      currency: "EUR",
      unit: "month",
      interval: "1",
      firstPayment: "2025-01-31",
    });

    // Export : l'enveloppe contient l'abonnement créé.
    await app.exportData();
    await expect.poll(() => app.exportedBundle()).toContain("Netflix");

    // Import d'une enveloppe avec une catégorie valide et un abonnement invalide (devise inconnue) :
    // la ligne fautive est rejetée, le rapport s'affiche.
    const bundle = JSON.stringify({
      version: 1,
      categories: [{ name: "ImportedCat" }],
      subscriptions: [
        {
          name: "BadSub",
          amount: "1.00",
          currency: "XXX",
          cycle: { unit: "month", interval: 1 },
          first_payment: "2025-01-01",
        },
      ],
    });
    await app.importData(bundle);
    expect(await app.importCreatedShown()).toBe(true);
    await expect.poll(() => app.importRejectedCount()).toBe(1);
  });
});
