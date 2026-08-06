import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SYN-008 — Appairage et synchronisation initiale. Conception subtrack (offline-first) : un appareil
// récupère l'intégralité des données via le delta paginé (REQ-SYN-003), de façon REPRENABLE. -> @design.
// Le versant jeton d'appareil (Bearer) est asseré en intégration ; cet e2e vérifie la reprenabilité à
// travers une interruption (rechargement de la page), le curseur étant persisté par lot.
test.describe("Synchronisation initiale reprenable", { tag: ["@design", "@REQ-SYN-008"] }, () => {
  test("reprend après un rechargement, sans repartir de zéro et sans doublon", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `syn008-${stamp}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Trois payeurs, en plus des 16 catégories par défaut : de quoi couvrir plusieurs pages.
    for (const name of ["Alex", "Bob", "Cleo"]) {
      await app.createPayer(name);
      expect(await app.payerVisible(name)).toBe(true);
    }

    // Premier pas de synchronisation initiale (page de 2), puis INTERRUPTION (rechargement) : le curseur
    // persisté doit survivre.
    const all: string[] = [];
    const step1 = await app.initialSyncStep(2);
    all.push(...step1.ids);
    expect(step1.hasMore).toBe(true);

    await page.reload();

    // Reprise après rechargement : on continue de draîner jusqu'à épuisement.
    let more = true;
    let guard = 0;
    while (more && guard < 50) {
      const step = await app.initialSyncStep(2);
      all.push(...step.ids);
      more = step.hasMore;
      guard += 1;
    }

    // Aucun doublon entre les pages (reprise propre au curseur, pas de recouvrement).
    expect(new Set(all).size).toBe(all.length);
    // Couverture complète : les trois payeurs créés sont présents.
    expect(all.filter((k) => k.startsWith("payer:")).length).toBe(3);
  });
});
