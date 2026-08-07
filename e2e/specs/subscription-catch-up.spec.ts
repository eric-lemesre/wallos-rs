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

    // Ancrage mensuel ~5 mois dans le passé : 5 occurrences dépassées d'un coup. Jour 15 :
    // aucun clamp de fin de mois en jeu, la génération des candidats (setMonth) reste exacte.
    const anchorMonth = new Date(Date.now() - 150 * 24 * 60 * 60 * 1000).toISOString().slice(0, 7);
    const anchor = `${anchorMonth}-15`;
    const today = new Date().toISOString().slice(0, 10);
    await app.computeNextDue(anchor, "month", "1", today);

    const result = (await app.readNextDue()).trim();
    // La date est affichée LOCALISÉE (REQ-I18N-003, langue en par défaut du navigateur de test) :
    // on reconstruit le format attendu pour chaque jour candidat futur (le jour d'ancrage du mois
    // courant ou du suivant) et on vérifie que l'un d'eux est affiché.
    const anchorDate = new Date(`${anchor}T00:00:00`);
    const format = (d: Date) =>
      new Intl.DateTimeFormat("en", { dateStyle: "medium" }).format(d);
    const candidates: string[] = [];
    for (let offset = 0; offset <= 12; offset += 1) {
      const c = new Date(anchorDate);
      c.setMonth(c.getMonth() + offset);
      if (c.toISOString().slice(0, 10) > today) {
        candidates.push(format(c));
      }
    }
    expect(
      candidates.some((c) => result.includes(c)),
      `« ${result} » devrait contenir une occurrence future parmi ${candidates.join(" | ")}`,
    ).toBe(true);
  });
});
