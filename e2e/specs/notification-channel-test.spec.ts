import { expect, test } from "@playwright/test";

import { TargetDriver } from "../drivers/TargetDriver";

const PASSWORD = "correct horse battery staple";

// REQ-NOT-006 — Test d'un canal de notification. L'oracle legacy (endpoints test*notifications.php)
// renvoie { success, message } après un envoi de test ; subtrack teste le canal ENREGISTRÉ et affiche
// un diagnostic localisé à partir d'un code stable (ADR 0047). Cet e2e vérifie le parcours UI : le
// résultat du test s'affiche, avec un diagnostic exploitable en cas d'échec (critère #1). La cible
// est volontairement injoignable : aucun récepteur externe n'est requis, et le cas de succès est
// couvert en intégration (récepteur local, garde SSRF oblige).
test.describe("Test d'un canal de notification", { tag: ["@legacy", "@REQ-NOT-006"] }, () => {
  test("le test d'un canal injoignable affiche un diagnostic d'échec", async ({ page, baseURL }) => {
    const app = new TargetDriver(page, baseURL!);
    const stamp = Date.now();
    const email = `not006-${stamp}@example.com`;

    await app.gotoSignup();
    await app.signup({ email, password: PASSWORD });
    expect(await app.signupSucceeded()).toBe(true);
    await app.login({ email, password: PASSWORD });
    expect(await app.currentUserVisible()).toBe(true);

    // URL publique syntaxiquement valide (passe la garde SSRF) mais injoignable (TLD réservé
    // `.invalid`, RFC 6761 : la résolution échoue toujours, sans dépendance réseau externe).
    await app.addWebhookChannel(`https://unreachable-${stamp}.invalid/hook`);

    // Le clic attend l'apparition de la ligne du canal (auto-wait Playwright).
    await app.testFirstChannel();
    // Le résultat s'affiche avec un diagnostic d'échec non vide (jamais l'erreur brute).
    await expect.poll(async () => (await app.channelTestResult())?.ok).toBe(false);
    const result = await app.channelTestResult();
    expect(result?.message.length).toBeGreaterThan(0);
    expect(result?.message).not.toContain(".invalid");
  });
});
