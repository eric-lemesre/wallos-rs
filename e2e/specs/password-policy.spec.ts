import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

test.describe("Politique de mot de passe", { tag: ["@design", "@REQ-AUT-003"] }, () => {
  test("refuse un mot de passe compromis à l'inscription", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    await app.gotoSignup();
    // Assez long (>= 12) mais figurant dans la liste de compromis.
    await app.signup({ email: `e2e-${Date.now()}@example.com`, password: "password1234" });
    expect(await app.hasPasswordError()).toBe(true);
    expect(await app.signupSucceeded()).toBe(false);
  });
});
