import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-SUB-014 — Rattrapage des échéances passées. `oracle: legacy` (cron updatenextpayment.php :
// « add intervals until future ») : subtrack reproduit la convergence via `core::next_due` (borne
// anti-pathologique en plus, ADR 0050). Un abonnement ancré plusieurs cycles dans le passé expose
// toujours une échéance STRICTEMENT future — jamais une date passée, jamais de rafale rétroactive
// (garde NOT-002, couverte en intégration).
test.describe("Rattrapage des échéances passées", { tag: ["@legacy", "@REQ-SUB-014"] }, () => {
  test("un ancrage vieux de plusieurs cycles donne une échéance future", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sub014-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Ancrage mensuel ~5 mois dans le passé : 5 occurrences dépassées d'un coup.
    const anchor = new Date(Date.now() - 150 * 24 * 60 * 60 * 1000).toISOString().slice(0, 10);
    const today = new Date().toISOString().slice(0, 10);
    await app.computeNextDue(anchor, "month", "1", today);

    const result = (await app.readNextDue()).trim();
    const shown = result.match(/\d{4}-\d{2}-\d{2}/)?.[0];
    expect(shown, `date attendue dans « ${result} »`).toBeTruthy();
    // Strictement postérieure à aujourd'hui : le rattrapage a sauté toutes les occurrences passées.
    expect(shown! > today).toBe(true);
  });
});
