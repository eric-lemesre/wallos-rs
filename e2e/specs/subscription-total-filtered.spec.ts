import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-STA-007 — cohérence entre le total et le filtre appliqué. subtrack indique explicitement que le
// total porte sur l'ensemble filtré (Wallos ne le fait pas -> divergence assumée) -> @design.
test.describe("Total et filtre actif", { tag: ["@design", "@REQ-STA-007"] }, () => {
  test("le total indique explicitement qu'il porte sur l'ensemble filtré", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta007-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.createSubscription({
      name: "Netflix", amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: "2030-01-31",
    });
    await app.awaitSubscriptions(["Netflix"]);

    // Sans filtre : aucune indication de filtre sur le total.
    expect(await app.totalIndicatesFiltered()).toBe(false);

    // Un filtre appliqué : le total l'indique explicitement.
    await app.filterSubscriptionsByState("active");
    expect(await app.totalIndicatesFiltered()).toBe(true);
  });
});
