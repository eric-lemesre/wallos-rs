import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

test.describe("Isolation des comptes", { tag: ["@design", "@REQ-SEC-001"] }, () => {
  test("un compte ne voit pas les appareils d'un autre foyer", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const alice = `iso-alice-${stamp}@example.com`;
    const bob = `iso-bob-${stamp}@example.com`;

    // Alice crée un compte et appaire un appareil.
    await app.gotoSignup();
    await app.signup({ email: alice, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: alice, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await app.pairDevice({ email: alice, password: PASSWORD }, "Alice-Laptop", "desktop");
    await app.openDevices();
    expect(await app.deviceListed("Alice-Laptop")).toBe(true);

    // Bob crée un compte puis se connecte : son login remplace le cookie d'Alice — inutile de se
    // déconnecter via l'UI (l'état du formulaire est d'ailleurs réinitialisé par le reload ci-dessus).
    // L'appareil d'Alice ne doit jamais apparaître chez Bob (isolation par foyer).
    await app.signup({ email: bob, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: bob, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await app.openDevices();
    expect(await app.deviceListed("Alice-Laptop")).toBe(false);
  });
});
