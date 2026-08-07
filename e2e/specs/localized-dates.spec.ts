import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-I18N-003 — Formats de date localisés (oracle design). La date suit la locale active (dérivée
// de la langue de l'interface, ADR 0051), jamais un format codé en dur ; la garde de fuseau (date
// civile reconstruite en local, pas d'interprétation UTC) est couverte par les tests unitaires du
// helper. Même donnée, deux langues → deux formats, même jour.
test.describe("Formats de date localisés", { tag: ["@design", "@REQ-I18N-003"] }, () => {
  test("la même échéance s'affiche en format anglais puis français", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const email = `i18n003-${Date.now()}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);
    await page.reload();

    // Le calculateur d'échéance affiche une date fixe connue (déterministe).
    await app.setLanguage("en");
    await app.computeNextDue("2025-01-31", "month", "1", "2025-01-31");
    await expect.poll(async () => await app.readNextDue()).toContain("Feb 28, 2025");

    // Français : même jour civil, conventions françaises.
    await app.setLanguage("fr");
    await app.computeNextDue("2025-01-31", "month", "1", "2025-01-31");
    // Regex tolérante à l'abréviation ICU de février (« févr. »/« fév. », revue I18N-003 F10).
    await expect.poll(async () => await app.readNextDue()).toMatch(/28 févr?\.? 2025/);
  });
});
