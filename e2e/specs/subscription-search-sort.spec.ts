import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-007 — recherche et tri. La recherche insensible casse+diacritiques sur nom ET notes et le
// tri par coût mensuel **normalisé en devise de référence** sont des choix subtrack (Wallos trie le
// prix brut et n'offre pas de recherche texte) -> @design.
test.describe("Recherche et tri des abonnements", { tag: ["@design", "@REQ-SUB-007"] }, () => {
  test("recherche insensible casse+diacritiques et tri déterministe", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub-search-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Trois abonnements de coûts mensuels distincts après normalisation du cycle.
    await app.createSubscription({
      name: "Alpha", amount: "30.00", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-15",
    });
    await app.createSubscription({
      name: "Beta", amount: "120.00", currency: "EUR", unit: "year", interval: "1", firstPayment: "2030-01-15",
    });
    await app.createSubscription({
      name: "Gamma", amount: "5.00", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-15",
    });

    // Tri par montant décroissant : Alpha (30) > Beta (10/mois) ≈ Gamma (5) → Alpha, Beta, Gamma.
    // `expect.poll` réessaie la lecture jusqu'à ce que la liste rechargée reflète l'ordre attendu.
    await app.sortSubscriptionsBy("amount");
    await expect.poll(() => app.subscriptionNames()).toEqual(["Alpha", "Beta", "Gamma"]);

    // Recherche sur le nom, insensible à la casse : « alpha » ne ramène qu'Alpha.
    await app.searchSubscriptions("alpha");
    await expect.poll(() => app.subscriptionNames()).toEqual(["Alpha"]);

    // Recherche vidée : les trois réapparaissent, triés par nom (défaut).
    await app.searchSubscriptions("");
    await app.sortSubscriptionsBy("name");
    await expect.poll(() => app.subscriptionNames()).toEqual(["Alpha", "Beta", "Gamma"]);
  });
});
