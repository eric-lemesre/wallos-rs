import { expect, test } from "@playwright/test";

import { createHarness } from "../drivers/Harness";

// Smoke de FONDATION du harnais legacy : prouve que (a) docker-compose démarre Wallos 5.4.2 épinglé
// par digest, et (b) le `LegacyDriver` pilote l'UI réelle (inscription + login → tableau de bord).
// Ce n'est pas un scénario d'exigence : il valide l'infrastructure, pas un comportement métier.
test.describe("Harnais legacy — smoke", { tag: ["@legacy-smoke"] }, () => {
  test("Wallos démarre et le LegacyDriver ouvre une session", async ({ page, baseURL }) => {
    const app = createHarness(page, baseURL!, "legacy");
    await app.signIn({ email: "e2e@example.com", password: "correcthorsebatterystaple" });
    expect(await app.signedIn()).toBe(true);
  });
});
