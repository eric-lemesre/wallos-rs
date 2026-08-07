import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";
// Secret d'opérateur injecté au serveur par playwright.config.ts (env CRON_TOKEN).
const CRON_TOKEN = "e2e-cron-token";

// REQ-NOT-002 — Idempotence de l'ordonnanceur (oracle design, layer [core, api] : scénario de
// contrat API, pas d'UI). Une occurrence déjà émise ne repart jamais, y compris si l'ordonnanceur
// est ré-exécuté (redémarrage, double déclenchement) : le journal `reminder_log` persiste et
// l'unicité `(foyer, abonnement, échéance, type)` sérialise. La garantie multi-instances et la
// garde anti-rétroactive sont couvertes en intégration (courses contrôlées).
test.describe("Idempotence de l'ordonnanceur", { tag: ["@design", "@REQ-NOT-002"] }, () => {
  test("une seconde exécution du cron n'émet aucun rappel déjà émis", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `not002-${stamp}@example.com`;
    const name = `Netflix ${stamp}`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // Le cron balaie TOUS les foyers de la base partagée : pour que `second === 0` soit
    // déterministe malgré les specs concurrents (qui créent des abonnements échéant demain),
    // on ancre l'abonnement sur une date FICTIVE éloignée dont le jour du mois diffère
    // toujours de celui de « demain » réel — aucune occurrence tierce ne tombe dans la fenêtre.
    const tomorrowDay = new Date(Date.now() + 24 * 60 * 60 * 1000).getUTCDate();
    const fictiveDay = (tomorrowDay % 28) + 1; // toujours != tomorrowDay, borné 1..28 (revue F5)
    const due = `2033-06-${String(fictiveDay).padStart(2, "0")}`;
    const asOf = new Date(new Date(`${due}T00:00:00Z`).getTime() - 24 * 60 * 60 * 1000)
      .toISOString()
      .slice(0, 10);
    await app.createSubscription({
      name, amount: "9.99", currency: "EUR", unit: "month", interval: "1", firstPayment: due,
    });
    // ATTENDRE la persistance avant de lancer le cron : la création part de l'UI et le cron
    // s'exécute côté serveur — sans cette barrière, le run1 peut précéder le commit et le run2
    // « découvre » alors l'abonnement (second == 1, flake observé en local). Le rechargement
    // force la liste à relire le serveur (préuve de persistance).
    await page.reload();
    await expect.poll(() => app.subscriptionNames()).toContain(name);

    // Première exécution : au moins le rappel de cet abonnement est émis.
    const first = await app.runReminders(asOf, CRON_TOKEN);
    expect(first).toBeGreaterThanOrEqual(1);
    // Ré-exécution (même fenêtre) : rien ne repart — le journal a retenu chaque occurrence.
    const second = await app.runReminders(asOf, CRON_TOKEN);
    expect(second).toBe(0);
    // Preuve relative (revue F3) : un troisième run immédiat, sans aucune création entre les
    // deux, reste à zéro — l'idempotence ne dépend pas d'un hasard de planification.
    const third = await app.runReminders(asOf, CRON_TOKEN);
    expect(third).toBe(0);
  });
});
