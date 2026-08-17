import type { Page } from "@playwright/test";

import { canonicalAmount } from "./amount";
import type { Credentials, Harness, SubscriptionInput } from "./Harness";

/**
 * Pilote l'application d'origine **Wallos 5.4.2** (cible figée, ADR 0011) via ses sélecteurs réels.
 * Wallos se connecte par **nom d'utilisateur** (dérivé ici de l'e-mail) ; à la première visite, aucun
 * compte n'existe → un formulaire d'**inscription** est présenté, puis le formulaire de **login**.
 *
 * Périmètre : authentification, puis les opérations métier nécessaires à la PREUVE des oracles
 * (ADR 0059/0060, R11). Chaque méthode ajoutée ici permet de promouvoir un oracle du niveau `read`
 * ou `executed` au niveau `observed` — le comportement constaté sur l'application en marche, et non
 * déduit de la lecture de son code.
 */
export class LegacyDriver implements Harness {
  constructor(
    private readonly page: Page,
    private readonly baseURL: string,
  ) {}

  /** Nom d'utilisateur Wallos dérivé de l'e-mail (Wallos ne se connecte pas par e-mail). */
  private username(email: string): string {
    return email.replace(/[^a-zA-Z0-9]/g, "").slice(0, 30) || "e2euser";
  }

  async signIn({ email, password }: Credentials): Promise<void> {
    const username = this.username(email);
    await this.page.goto(`${this.baseURL}/login.php`);

    // Première visite (aucun compte) : le formulaire d'inscription est affiché.
    if ((await this.page.locator("#confirm_password").count()) > 0) {
      await this.page.fill("#username", username);
      await this.page.fill("#email", email);
      await this.page.fill("#password", password);
      await this.page.fill("#confirm_password", password);
      // Les selects `#currency` (défaut EUR) et `#language` (défaut en) ont déjà une valeur valide
      // sélectionnée — on garde les défauts (le smoke ne teste pas la devise).
      await this.page.locator('input[value="Register"]').click();
      await this.page.waitForURL(/login\.php/);
    }

    // Formulaire de login (nom d'utilisateur + mot de passe).
    await this.page.fill("#username", username);
    await this.page.fill("#password", password);
    await this.page.locator('input[value="Login"]').click();
    await this.page.waitForURL((url) => !url.pathname.endsWith("login.php"), {
      timeout: 15_000,
    });
  }

  async signedIn(): Promise<boolean> {
    // `/` redirige vers `subscriptions.php` si authentifié, sinon vers `login.php`. On teste l'URL
    // finale (robuste : indépendant de la visibilité d'un menu).
    await this.page.goto(`${this.baseURL}/`, { waitUntil: "load" });
    return !this.page.url().includes("login.php");
  }

  /** Ferme la session Wallos. */
  async signOut(): Promise<void> {
    await this.page.goto(`${this.baseURL}/logout.php`);
  }

  /**
   * Cycles de Wallos : 1 = jours, 2 = semaines, 3 = mois, 4 = années, 5 = paiement unique.
   * Correspondance capturée sur l'image épinglée (`REQ-STA-001-monthly.json`, `_cycle_mapping`).
   */
  private static readonly CYCLE: Record<SubscriptionInput["unit"], string> = {
    day: "1",
    week: "2",
    month: "3",
    year: "4",
  };

  async createSubscription(input: SubscriptionInput): Promise<void> {
    await this.page.goto(`${this.baseURL}/subscriptions.php`);
    // Le formulaire est un panneau masqué qu'ouvre `addSubscription()`. On appelle la fonction
    // plutôt que de cliquer : le bouton porte une icône, et son libellé accessible varie avec la
    // langue de l'instance — l'appel direct est stable, le clic ne l'est pas.
    await this.page.evaluate(() => {
      (window as unknown as { addSubscription?: () => void }).addSubscription?.();
    });
    await this.page.waitForSelector("#subscription-form", { state: "visible", timeout: 10_000 });

    await this.page.fill("#name", input.name);
    await this.page.fill("#price", input.amount);
    await this.page.selectOption("#cycle", LegacyDriver.CYCLE[input.unit]);
    await this.page.selectOption("#frequency", input.interval);
    await this.page.fill("#next_payment", input.firstPayment);
    await this.page.locator("#save-button").click();
    // Wallos recharge la liste après enregistrement ; on attend la disparition du panneau.
    await this.page.waitForSelector("#subscription-form", { state: "hidden", timeout: 10_000 });
  }

  async readMonthlyTotal(): Promise<string> {
    await this.page.goto(`${this.baseURL}/stats.php`, { waitUntil: "load" });
    // La page rend une série de vignettes « <montant> <libellé> ». On vise la CLASSE EXACTE
    // `statistic` : un sélecteur approchant (`[class*=stat]`) attrape aussi le conteneur englobant,
    // dont le texte agrège toutes les vignettes — l'extraction rendait alors un nombre composite.
    // Le libellé « Monthly Cost » identifie la vignette sans ambiguïté : « Average Monthly
    // Subscription Cost » ne le contient pas (« Monthly Subscription Cost »).
    const rendu = await this.page
      .locator("div.statistic")
      .filter({ hasText: /Monthly Cost/ })
      .first()
      .innerText();
    return canonicalAmount(rendu.split("Monthly Cost")[0] ?? rendu);
  }
}
