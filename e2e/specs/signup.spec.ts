import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

test.describe("Inscription", { tag: ["@design", "@REQ-AUT-001"] }, () => {
  test("crée un compte avec un mot de passe conforme", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    await app.gotoSignup();
    await app.signup({
      email: `e2e-${Date.now()}@example.com`,
      password: "correct horse battery staple",
    });
    expect(await app.signupSucceeded()).toBe(true);
  });

  test("refuse un mot de passe trop court, sans soumettre", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    await app.gotoSignup();
    await app.signup({ email: "e2e-short@example.com", password: "short" });
    expect(await app.hasPasswordError()).toBe(true);
    expect(await app.signupSucceeded()).toBe(false);
  });
});
