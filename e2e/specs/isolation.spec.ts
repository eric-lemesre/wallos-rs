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
    await app.logout();

    // Bob crée un compte, se connecte : l'appareil d'Alice n'apparaît jamais chez lui.
    await app.signup({ email: bob, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email: bob, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await app.openDevices();
    expect(await app.deviceListed("Alice-Laptop")).toBe(false);
  });
});
