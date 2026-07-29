import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// Oracle gelé (§8.1) : facteurs de normalisation mensuelle CAPTURÉS sur Wallos 5.4.2 (getPricePerMonth),
// vérifiés en exécutant le PHP de l'image pinnée. `displayed_eur` = valeur affichée (arrondi EUR, CUR-005).
const oracle = JSON.parse(
  readFileSync(
    join(dirname(fileURLToPath(import.meta.url)), "..", "fixtures", "oracles", "REQ-STA-001-monthly.json"),
    "utf-8",
  ),
) as {
  vectors: { unit: string; interval: number; price: string; displayed_eur: string; why: string }[];
};

// REQ-STA-001 — normalisation du coût mensuel. La méthode (facteur par cycle) est une CONVENTION capturée
// sur l'application d'origine -> @legacy.
test.describe("Coût mensuel normalisé", { tag: ["@legacy", "@REQ-STA-001"] }, () => {
  // Échantillon représentatif des cycles (annuel, hebdo, mensuel multi-intervalle) pour garder l'e2e court.
  const sample = oracle.vectors.filter((v) =>
    [
      "year:1:120.00",
      "week:1:10.00",
      "week:2:10.00",
      "month:3:15.00",
    ].includes(`${v.unit}:${v.interval}:${v.price}`),
  );

  test("affiche le coût mensuel exactement comme l'application d'origine", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta001-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    for (const [i, v] of sample.entries()) {
      const name = `Sub-${v.unit}-${v.interval}-${i}`;
      await app.createSubscription({
        name,
        amount: v.price,
        currency: "EUR",
        unit: v.unit,
        interval: String(v.interval),
        firstPayment: "2030-01-31",
      });
      // Le coût mensuel affiché correspond EXACTEMENT à la valeur de l'oracle (facteur capturé + arrondi EUR).
      expect(await app.subscriptionMonthlyCost(name)).toContain(v.displayed_eur);
    }
  });
});
