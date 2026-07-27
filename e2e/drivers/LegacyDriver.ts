import type { Page } from "@playwright/test";

import type { Credentials, Harness } from "./Harness";

/**
 * Pilote l'application d'origine **Wallos 5.4.2** (cible figée, ADR 0011) via ses sélecteurs réels.
 * Wallos se connecte par **nom d'utilisateur** (dérivé ici de l'e-mail) ; à la première visite, aucun
 * compte n'existe → un formulaire d'**inscription** est présenté, puis le formulaire de **login**.
 *
 * Périmètre « fondation » : authentification uniquement. Les opérations métier (abonnements, devises,
 * catégories) seront ajoutées avec la première exigence `oracle: legacy`.
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
}
