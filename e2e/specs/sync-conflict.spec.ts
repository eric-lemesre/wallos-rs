import { randomUUID } from "node:crypto";

import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-005 — Résolution de conflit. Conception subtrack (dernière écriture gagnante + journal des
// versions perdues), sans équivalent Wallos -> @design. L'arbitrage est asseré en intégration ; cet e2e
// vérifie le parcours client : un écrasement fondé sur une version périmée journalise la version perdue,
// consultable.
test.describe("Résolution de conflit", { tag: ["@design", "@REQ-SYN-005"] }, () => {
  test("un écrasement concurrent conserve la version perdue au journal", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `syn005-${Date.now()}@example.com`;
    const payerId = randomUUID(); // base e2e partagée, clé primaire globale

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // v0 puis écriture fondée sur une version périmée (base ancienne) : conflit d'écrasement.
    expect(
      await app.pushSyncChanges([
        { op: "upsert", entity_type: "payer", id: payerId, payload: { name: "Alex" } },
      ]),
    ).toEqual(["applied"]);
    expect(
      await app.pushSyncChanges([
        {
          op: "upsert",
          entity_type: "payer",
          id: payerId,
          payload: { name: "Alexandra" },
          base_version: "2000-01-01T00:00:00Z",
        },
      ]),
    ).toEqual(["applied"]);

    // La version écrasée est conservée au journal des conflits.
    await expect.poll(() => app.conflictReasons()).toContain("overwritten");
  });
});
