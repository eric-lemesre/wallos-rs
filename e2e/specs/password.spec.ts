import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const OLD_PASSWORD = "correct horse battery staple";
const NEW_PASSWORD = "totally fresh secret passphrase";

test.describe("Changement de mot de passe", { tag: ["@design", "@REQ-AUT-007"] }, () => {
  test("change le mot de passe puis l'ancien ne connecte plus", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-pw-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: OLD_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);

    await app.login({ email, password: OLD_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    await app.changePassword(OLD_PASSWORD, NEW_PASSWORD);
    expect(await app.passwordChangeSucceeded()).toBe(true);

    // L'ancien mot de passe ne connecte plus ; le nouveau, oui.
    await app.logout();
    await app.login({ email, password: OLD_PASSWORD });
    expect(await app.loginFailed()).toBe(true);

    await app.login({ email, password: NEW_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
  });
});
