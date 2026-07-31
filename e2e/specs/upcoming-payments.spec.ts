import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

/** Date ISO (YYYY-MM-DD) décalée de `days` jours par rapport à aujourd'hui. */
function isoInDays(days: number): string {
  const d = new Date();
  d.setUTCDate(d.getUTCDate() + days);
  return d.toISOString().slice(0, 10);
}

// REQ-STA-005 — échéancier des prochains paiements. `oracle: legacy` mais le modèle diffère
// délibérément de Wallos (fenêtre de N jours ancrée sur « aujourd'hui » vs calendrier mensuel PHP ;
// occurrences ancrées+clampées, ADR 0022, pas le débordement Wallos) -> scénario @design. La fenêtre
// utilise l'horloge serveur : on ancre donc l'abonnement sur des dates relatives à aujourd'hui.
test.describe("Échéancier des prochains paiements", { tag: ["@design", "@REQ-STA-005"] }, () => {
  test("liste les occurrences attendues d'un abonnement actif dans la fenêtre", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta005-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Abonnement mensuel dont le premier paiement échoit dans 3 jours : sur une fenêtre de 90 jours
    // (~3 mois), on attend au moins deux occurrences (mois M et M+1) — plusieurs occurrences d'un même
    // abonnement, cœur de l'acceptance STA-005.
    const name = `Échéancier ${Date.now()}`;
    await app.createSubscription({
      name,
      amount: "9.99",
      currency: "EUR",
      unit: "month",
      interval: "1",
      firstPayment: isoInDays(3),
    });

    await app.loadUpcoming("90");
    expect(await app.upcomingOccurrences(name)).toBeGreaterThanOrEqual(2);
  });

  test("un abonnement dont le premier paiement est hors fenêtre n'apparaît pas", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `sta005-out-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Premier paiement dans ~2 ans : hors d'une fenêtre de 30 jours -> aucune occurrence listée.
    const name = `Lointain ${Date.now()}`;
    await app.createSubscription({
      name,
      amount: "5.00",
      currency: "EUR",
      unit: "year",
      interval: "1",
      firstPayment: isoInDays(730),
    });

    await app.loadUpcoming("30");
    expect(await app.upcomingOccurrences(name)).toBe(0);
  });
});
