import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// Oracle gelé (§8.1) : relation annuel = mensuel × 12 CAPTURÉE sur Wallos 5.4.2 (stats_calculations.php),
// vérifiée en exécutant le PHP de l'image pinnée. `displayed_eur` = valeur affichée (arrondi EUR).
const oracle = JSON.parse(
  readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures", "oracles", "REQ-STA-002-yearly.json"),
    "utf-8",
  ),
) as {
  vectors: { unit: string; interval: number; price: string; displayed_eur: string; why: string }[];
};

// REQ-STA-002 — coût annuel normalisé. La relation (× 12) est capturée sur l'application d'origine -> @legacy.
test.describe("Coût annuel normalisé", { tag: ["@legacy", "@REQ-STA-002"] }, () => {
  const sample = oracle.vectors.filter((v) =>
    ["year:1:120.00", "week:1:10.00", "month:3:15.00"].includes(`${v.unit}:${v.interval}:${v.price}`),
  );

  test("affiche le coût annuel exactement comme l'application d'origine", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta002-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    const names = sample.map((v, i) => `Yr-${v.unit}-${v.interval}-${i}`);
    for (const [i, v] of sample.entries()) {
      await app.createSubscription({
        name: names[i],
        amount: v.price,
        currency: "EUR",
        unit: v.unit,
        interval: String(v.interval),
        firstPayment: "2030-01-31",
      });
    }
    // Poll AVEC re-rafraîchissement : le premier refresh peut précéder le commit de la dernière
    // création (course UI -> serveur, même flake que le spec de coût mensuel).
    await expect
      .poll(async () => {
        await app.refreshSubscriptions();
        return app.subscriptionNames();
      })
      .toEqual(expect.arrayContaining(names));
    for (const [i, v] of sample.entries()) {
      expect(await app.subscriptionYearlyCost(names[i])).toContain(v.displayed_eur);
    }
  });
});
