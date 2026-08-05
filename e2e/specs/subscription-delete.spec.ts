import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-005 — Suppression d'un abonnement. Conception subtrack (suppression TRAÇABLE via pierre
// tombale, REQ-SYN-002, pour la réplication multi-appareils) -> @design. La création de la pierre
// tombale est asserée en intégration ; cet e2e vérifie le parcours UI : suppression -> disparition de
// la vue, puis exposition à la synchronisation.
test.describe("Suppression d'abonnement", { tag: ["@design", "@REQ-SUB-005"] }, () => {
  test("supprime l'abonnement, le retire de la vue et l'expose à la synchronisation", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub005-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2030-01-15",
    });
    // Le formulaire et la liste sont indépendants : on déclenche un rechargement (« Appliquer ») pour
    // que la liste reflète la création.
    await app.searchSubscriptions("");
    await expect.poll(() => app.subscriptionNames()).toContain("Netflix");

    // Supprime : l'abonnement disparaît de la liste...
    await app.deleteSubscription("Netflix");
    await expect.poll(() => app.subscriptionNames()).not.toContain("Netflix");

    // ...et la suppression est exposée comme pierre tombale à la synchronisation.
    await expect.poll(() => app.tombstonedEntityTypes()).toContain("subscription");
  });
});
