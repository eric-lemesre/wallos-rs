import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const STRONG_PASSWORD = "correct horse battery staple";

test.describe("Authentification", { tag: ["@design", "@REQ-AUT-002"] }, () => {
  test("la connexion après inscription donne accès au compte", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: STRONG_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);

    await app.login({ email, password: STRONG_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
  });

  test("un mauvais mot de passe affiche un message générique sans connecter", async ({
    page,
    baseURL,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    await app.gotoSignup();
    await app.login({ email: "nobody@example.com", password: "wrong password" });
    expect(await app.loginFailed()).toBe(true);
    expect(await app.currentUserVisible()).toBe(false);
  });
});
