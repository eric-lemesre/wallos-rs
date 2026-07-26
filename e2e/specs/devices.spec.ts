import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const STRONG_PASSWORD = "correct horse battery staple";

test.describe("Gestion des appareils", { tag: ["@design", "@REQ-AUT-006"] }, () => {
  test("liste un appareil appairé puis le révoque", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-dev-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: STRONG_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);

    await app.login({ email, password: STRONG_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Un appareil natif s'appaire (jeton propre à l'appareil) ; la page web le liste.
    await app.pairDevice({ email, password: STRONG_PASSWORD }, "E2E Laptop", "desktop");
    await app.openDevices();
    expect(await app.deviceListed("E2E Laptop")).toBe(true);

    // La révocation le retire immédiatement de la liste.
    await app.revokeDevice("E2E Laptop");
    expect(await app.deviceGone("E2E Laptop")).toBe(true);
  });
});
