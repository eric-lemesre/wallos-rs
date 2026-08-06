import { randomUUID } from "node:crypto";

import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-004 — Poussée des modifications locales. Endpoint de conception subtrack (offline-first),
// sans équivalent Wallos -> @design. Application partielle et idempotence sont asserées en intégration ;
// cet e2e vérifie le parcours client : pousser un lot, puis le rejouer à l'identique (reprise après
// coupure) sans effet de bord supplémentaire.
test.describe("Poussée des modifications locales", { tag: ["@design", "@REQ-SYN-004"] }, () => {
  test("applique un lot puis tolère son rejeu à l'identique", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `syn004-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Id unique par exécution : la base e2e est partagée et la clé primaire des entités est globale.
    const payerId = randomUUID();
    const ops = [{ op: "upsert", entity_type: "payer", id: payerId, payload: { name: "Alex" } }];

    // Poussée initiale : appliquée.
    expect(await app.pushSyncChanges(ops, "batch-1")).toEqual(["applied"]);
    // Le payeur apparaît dans la liste après rechargement.
    await page.reload();
    expect(await app.payerVisible("Alex")).toBe(true);

    // Rejeu à l'identique (reprise après coupure) : même réponse, aucun doublon.
    expect(await app.pushSyncChanges(ops, "batch-1")).toEqual(["applied"]);
    await page.reload();
    // Assertion web-first (réessai) : un seul payeur « Alex », jamais un doublon.
    await expect(page.getByTestId("payer-row").filter({ hasText: "Alex" })).toHaveCount(1);
  });
});
