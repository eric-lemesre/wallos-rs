import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-007 — Fonctionnement hors ligne. Conception subtrack (modalité web responsive, OQ-009 : pas de
// coquille native) : une écriture effectuée sans réseau aboutit localement (ajout optimiste) et part en
// file (outbox), poussée automatiquement au retour de la connectivité. -> @design.
test.describe("Fonctionnement hors ligne", { tag: ["@design", "@REQ-SYN-007"] }, () => {
  test("crée hors ligne puis synchronise automatiquement au retour du réseau", async ({
    page,
    baseURL,
    context,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `syn007-${stamp}@example.com`;
    const payerName = `Alex ${stamp}`;

    // Connexion en ligne (le passage hors ligne bloque tout le réseau, y compris l'authentification).
    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await expect.poll(() => app.syncStatus()).toBe("synced");

    // Coupure réseau : l'indicateur passe « hors ligne ».
    await context.setOffline(true);
    await expect.poll(() => app.syncStatus()).toBe("offline");

    // Création hors ligne : l'opération aboutit localement (le payeur apparaît).
    await app.createPayer(payerName);
    expect(await app.payerVisible(payerName)).toBe(true);

    // Retour du réseau : synchronisation automatique, sans action de l'utilisateur.
    await context.setOffline(false);
    await expect.poll(() => app.syncStatus()).toBe("synced");

    // La création est persistée côté serveur : elle survit à un rechargement.
    await page.reload();
    expect(await app.payerVisible(payerName)).toBe(true);
  });
});
