import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-003 — Récupération incrémentale par curseur. Endpoint de conception subtrack (offline-first),
// sans équivalent Wallos -> @design. La correction de la pagination (ni omission ni duplication) est
// asserée en intégration ; cet e2e vérifie le parcours client : créer des entités puis draîner tout le
// delta par petites pages, sans doublon.
test.describe("Récupération incrémentale par curseur", { tag: ["@design", "@REQ-SYN-003"] }, () => {
  test("draine tout le delta par pages successives sans duplication", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `syn003-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Trois payeurs, en plus des 16 catégories par défaut : de quoi dépasser une petite taille de page.
    // On attend la confirmation de chaque création (liste rechargée) pour éviter une course avec le drain.
    for (const name of ["Alex", "Bob", "Cleo"]) {
      await app.createPayer(name);
      expect(await app.payerVisible(name)).toBe(true);
    }

    // Draine tout le delta avec des pages de 2 : la concaténation ne contient aucun doublon...
    const keys = await app.drainSyncChanges(2);
    expect(new Set(keys).size).toBe(keys.length);

    // ...et couvre bien les trois payeurs créés.
    const payerCount = keys.filter((k) => k.startsWith("payer:")).length;
    expect(payerCount).toBe(3);
  });
});
