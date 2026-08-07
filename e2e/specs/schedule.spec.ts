import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-012 — échéance mensuelle. `oracle: legacy` mais subtrack rejette délibérément le débordement
// PHP de Wallos (ADR 0022) au profit de l'ancrage+clamp -> scénario @design (fixture: les deux figés).
test.describe("Prochaine échéance mensuelle", { tag: ["@design", "@REQ-SUB-012"] }, () => {
  test("31 janvier échoit le 28 février (ancrage + clamp fin de mois)", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sched-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Abonnement mensuel démarré le 31 janvier : la prochaine échéance est le 28 février (clampée),
    // jamais un débordement au 3 mars (le bug PHP de Wallos, rejeté par ADR 0022).
    await app.computeNextDue("2025-01-31", "month", "1", "2025-01-31");
    expect(await app.readNextDue()).toContain("Feb 28, 2025");
  });
});
