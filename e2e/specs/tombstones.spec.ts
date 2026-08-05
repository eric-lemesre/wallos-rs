import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-002 — Pierres tombales. Comportement de conception subtrack (offline-first) : Wallos n'a pas
// de notion de trace de suppression pour la réplication -> @design. La logique de rétention/péremption
// est asserée en intégration (fenêtre injectée) ; cet e2e vérifie le parcours end-to-end : une
// suppression réelle dans l'UI est exposée comme pierre tombale par l'endpoint de synchronisation.
test.describe("Pierres tombales", { tag: ["@design", "@REQ-SYN-002"] }, () => {
  test("une suppression est exposée à la synchronisation", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `syn002-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Crée puis supprime un payeur (non référencé) via l'interface.
    await app.createPayer("Alex");
    expect(await app.payerVisible("Alex")).toBe(true);
    await app.deletePayer("Alex");

    // La suppression est reçue comme pierre tombale par l'endpoint de synchronisation.
    await expect.poll(() => app.tombstonedEntityTypes()).toContain("payer");
  });
});
