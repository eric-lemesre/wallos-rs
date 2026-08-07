import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-STA-004 — Répartition des coûts par catégorie ET par payeur. La mécanique d'agrégation (somme des
// parts = total) est capturée sur Wallos 5.4.2 (stats_calculations.php, categoryCost/memberCost) et gelée
// dans e2e/fixtures/oracles/REQ-STA-004-repartition.json ; l'invariant est asséré en intégration. La
// COMPOSITION par foyer et l'entrée explicite « (aucun) » (subtrack rend category_id/payer_id nullables,
// là où Wallos mono-foyer n'a que des sentinelles) ne sont pas rejouables sur Wallos -> @design.
test.describe("Répartition par catégorie et par payeur", { tag: ["@design", "@REQ-STA-004"] }, () => {
  test("affiche les deux axes, l'entrée « (aucun) », et un total = somme des parts", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta004-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Un abonnement 5 € sans catégorie ni payeur (créé d'abord, aucune catégorie sélectionnée)...
    await app.createSubscription({
      name: "Presse", amount: "5.00", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2020-01-15",
    });
    // ...puis une catégorie « Streaming » et un abonnement 10 € dedans (recharge pour peupler le sélecteur).
    await app.createCategory("Streaming");
    expect(await app.categoryVisible("Streaming")).toBe(true);
    await page.reload();
    await app.createSubscription({
      name: "Netflix", amount: "10.00", currency: "EUR", unit: "month", interval: "1",
      firstPayment: "2020-01-15", category: "Streaming",
    });

    // Poll AVEC re-rechargement : la seconde création peut ne pas être committée au premier
    // rechargement de la carte (course UI -> serveur, flake observé en suite parallèle).
    await expect
      .poll(async () => {
        await app.reloadRepartition();
        return app.repartitionGrandTotal();
      })
      .toContain("€15.00");

    // Axe catégorie : « Streaming » présent, et l'entrée « (aucun) » pour l'abonnement sans catégorie
    // (critère #2 : jamais omise). Locale par défaut de l'app de test = en -> « (none) ».
    await expect.poll(() => app.repartitionLabels("category")).toEqual(
      expect.arrayContaining(["Streaming", "(none)"]),
    );
    // Axe payeur : aucun abonnement n'a de payeur -> une unique entrée « (aucun) », jamais omise.
    await expect.poll(() => app.repartitionLabels("payer")).toEqual(["(none)"]);
  });
});
