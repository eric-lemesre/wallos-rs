import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const STRONG_PASSWORD = "correct horse battery staple";

test.describe("Déconnexion", { tag: ["@design", "@REQ-AUT-009"] }, () => {
  test("déconnecte et retire l'accès au compte", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: STRONG_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);

    await app.login({ email, password: STRONG_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.logout();
    expect(await app.currentUserGone()).toBe(true);
  });
});
