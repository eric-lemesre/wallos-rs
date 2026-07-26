import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const STRONG_PASSWORD = "correct horse battery staple";

test.describe("Session", { tag: ["@design", "@REQ-AUT-004"] }, () => {
  test("le cookie de session est HttpOnly et SameSite=Lax, sans donnée métier", async ({
    page,
    baseURL,
    context,
  }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `e2e-${Date.now()}@example.com`;
    await app.gotoSignup();
    await app.signup({ email, password: STRONG_PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: STRONG_PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    const session = (await context.cookies()).find((c) => c.name === "session");
    expect(session, "un cookie de session est posé").toBeDefined();
    expect(session?.httpOnly).toBe(true);
    expect(session?.sameSite).toBe("Lax");
    // Opaque : ne contient ni l'e-mail ni de donnée métier.
    expect(session?.value ?? "").not.toContain("@");
  });
});
