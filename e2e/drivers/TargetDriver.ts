import type { Page } from "@playwright/test";

import type { AppDriver, MoneyInput, SignupInput } from "./AppDriver";
import type { Credentials, Harness } from "./Harness";

/**
 * Pilote `subtrack` via les `data-testid` stables (les sélecteurs CSS/XPath sont interdits, §7).
 */
export class TargetDriver implements AppDriver, Harness {
  constructor(
    private readonly page: Page,
    private readonly baseURL: string,
  ) {}

  // --- Contrat agnostique partagé (Harness, §8.1) ---

  async signIn({ email, password }: Credentials): Promise<void> {
    await this.gotoSignup();
    await this.signup({ email, password });
    await this.login({ email, password });
  }

  async signedIn(): Promise<boolean> {
    return this.currentUserVisible();
  }

  async gotoSignup(): Promise<void> {
    await this.page.goto(this.baseURL);
  }

  async signup({ email, password }: SignupInput): Promise<void> {
    await this.page.getByTestId("signup-email").fill(email);
    await this.page.getByTestId("signup-password").fill(password);
    await this.page.getByTestId("signup-submit").click();
  }

  async signupSucceeded(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("signup-success")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async hasPasswordError(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("signup-password-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async login({ email, password }: SignupInput): Promise<void> {
    await this.page.getByTestId("login-email").fill(email);
    await this.page.getByTestId("login-password").fill(password);
    await this.page.getByTestId("login-submit").click();
  }

  async currentUserVisible(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-current-user")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async loginFailed(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async logout(): Promise<void> {
    await this.page.getByTestId("logout").click();
  }

  async currentUserGone(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("login-current-user")
        .waitFor({ state: "detached", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Gestion des appareils (REQ-AUT-006) ---

  async pairDevice(
    { email, password }: SignupInput,
    label: string,
    platform: string,
  ): Promise<void> {
    // Appairage via l'API depuis le contexte de la page (même origine + cookie de session),
    // comme le ferait une coquille native présentant email/mot de passe.
    await this.page.evaluate(
      async ([email, password, label, platform]) => {
        await fetch("/api/v1/device-sessions", {
          method: "POST",
          headers: { "content-type": "application/json" },
          body: JSON.stringify({ email, password, label, platform }),
        });
      },
      [email, password, label, platform] as const,
    );
  }

  async openDevices(): Promise<void> {
    await this.page.reload();
  }

  private deviceRow(label: string) {
    return this.page.getByTestId("device-row").filter({ hasText: label });
  }

  async deviceListed(label: string): Promise<boolean> {
    try {
      await this.deviceRow(label).waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async revokeDevice(label: string): Promise<void> {
    await this.deviceRow(label).getByRole("button").click();
  }

  async deviceGone(label: string): Promise<boolean> {
    try {
      await this.deviceRow(label).waitFor({ state: "detached", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Changement de mot de passe (REQ-AUT-007) ---

  async changePassword(current: string, next: string): Promise<void> {
    await this.page.getByTestId("change-password-current").fill(current);
    await this.page.getByTestId("change-password-new").fill(next);
    await this.page.getByTestId("change-password-submit").click();
  }

  async passwordChangeSucceeded(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("change-password-success")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Création d'abonnement (REQ-SUB-002) ---

  async createSubscription(input: {
    name: string; amount: string; currency: string; unit: string; interval: string; firstPayment: string; endDate?: string; category?: string;
  }): Promise<void> {
    await this.page.getByTestId("sub-name").fill(input.name);
    await this.page.getByTestId("sub-amount").fill(input.amount);
    await this.page.getByTestId("sub-currency").fill(input.currency);
    await this.page.getByTestId("sub-cycle-unit").selectOption(input.unit);
    await this.page.getByTestId("sub-cycle-interval").fill(input.interval);
    await this.page.getByTestId("sub-first-payment").fill(input.firstPayment);
    if (input.endDate !== undefined) {
      await this.page.getByTestId("sub-end-date").fill(input.endDate);
    }
    // Rattachement à une catégorie (par nom) : le sélecteur est peuplé depuis GET /categories.
    if (input.category !== undefined) {
      await this.page.getByTestId("sub-category").selectOption({ label: input.category });
    }
    await this.page.getByTestId("sub-submit").click();
  }

  async subscriptionEnded(name: string): Promise<boolean> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    try {
      await row.getByTestId("subscription-ended").waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionNextPayment(): Promise<string> {
    await this.page.getByTestId("sub-success").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("sub-success").textContent()) ?? "";
  }

  // --- Échéancier des prochains paiements (REQ-STA-005) ---

  async loadUpcoming(days: string): Promise<void> {
    await this.page.getByTestId("upcoming-days").fill(days);
    await this.page.getByTestId("upcoming-load").click();
  }

  /** Nombre d'occurrences de l'échéancier portant le nom `name`. */
  async upcomingOccurrences(name: string): Promise<number> {
    const list = this.page.getByTestId("upcoming-list");
    try {
      await list.waitFor({ state: "visible", timeout: 5000 });
    } catch {
      return 0;
    }
    return this.page
      .getByTestId("upcoming-row")
      .filter({ hasText: name })
      .count();
  }

  // --- Liste et filtres (REQ-SUB-006) ---

  async refreshSubscriptions(): Promise<void> {
    await this.page.getByTestId("subscriptions-apply").click();
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
  }

  async filterSubscriptionsByCategory(category: string): Promise<void> {
    await this.page.getByTestId("subscriptions-filter-category").fill(category);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  async subscriptionListed(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscription-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionsEmpty(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscriptions-empty")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async subscriptionsTotal(): Promise<string> {
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("subscriptions-total").textContent()) ?? "";
  }

  // --- Cohérence total / filtre (REQ-STA-007) ---

  async totalIndicatesFiltered(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("subscriptions-total-filtered")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async filterSubscriptionsByState(state: "all" | "active" | "inactive"): Promise<void> {
    await this.page.getByTestId("subscriptions-filter-state").selectOption(state);
    await this.page.getByTestId("subscriptions-apply").click();
  }

  // --- Modification (REQ-SUB-004) ---

  async editSubscriptionAmount(name: string, amount: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-edit").click();
    await row.getByTestId("subscription-amount-input").fill(amount);
    await row.getByTestId("subscription-save").click();
    // Attend que la liste rechargée reflète le nouveau montant (recalcul serveur appliqué).
    await this.page
      .getByTestId("subscription-amount")
      .filter({ hasText: amount })
      .first()
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async subscriptionAmount(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-amount").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-amount").textContent()) ?? "";
  }

  // --- Coût mensuel normalisé (REQ-STA-001) ---

  async subscriptionMonthlyCost(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-monthly").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-monthly").textContent()) ?? "";
  }

  async subscriptionYearlyCost(name: string): Promise<string> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-yearly").waitFor({ state: "visible", timeout: 5000 });
    return (await row.getByTestId("subscription-yearly").textContent()) ?? "";
  }

  // --- Abonnement désactivé (REQ-SUB-008) ---

  async deactivateSubscription(name: string): Promise<void> {
    const row = this.page.getByTestId("subscription-row").filter({ hasText: name });
    await row.getByTestId("subscription-edit").click();
    await row.getByTestId("subscription-active").uncheck();
    await row.getByTestId("subscription-save").click();
    // Attend que la liste rechargée reflète l'exclusion du total (total nul).
    await this.page.getByTestId("subscriptions-total").waitFor({ state: "visible", timeout: 5000 });
  }

  // --- Calcul d'échéance (REQ-SUB-012) ---

  async computeNextDue(anchor: string, unit: string, interval: string, after: string): Promise<void> {
    await this.page.getByTestId("nextdue-anchor").fill(anchor);
    await this.page.getByTestId("nextdue-unit").selectOption(unit);
    await this.page.getByTestId("nextdue-interval").fill(interval);
    await this.page.getByTestId("nextdue-after").fill(after);
    await this.page.getByTestId("nextdue-compute").click();
  }

  async readNextDue(): Promise<string> {
    await this.page.getByTestId("nextdue-result").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("nextdue-result").textContent()) ?? "";
  }

  // --- Catégories (REQ-CAT-001) ---

  async createCategory(name: string): Promise<void> {
    await this.page.getByTestId("category-new-name").fill(name);
    await this.page.getByTestId("category-create").click();
  }

  async categoryVisible(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("category-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async categoryAbsent(name: string): Promise<boolean> {
    // La liste doit être chargée (section visible) avant de conclure à l'absence.
    await this.page.getByTestId("categories-list").waitFor({ state: "visible", timeout: 5000 });
    return (
      (await this.page.getByTestId("category-name").filter({ hasText: name }).count()) === 0
    );
  }

  /** Clique « Supprimer » sur la ligne de la catégorie identifiée par son nom (REQ-CAT-003). */
  async deleteCategory(name: string): Promise<void> {
    const row = this.page.getByTestId("category-row").filter({ hasText: name });
    await row.getByTestId(/^category-delete-/).click();
  }

  /** Vrai si le refus de suppression (catégorie référencée, 409) est affiché (REQ-CAT-003). */
  async categoryDeleteRefused(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("category-delete-error")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  // --- Langue (REQ-I18N-001) ---

  async setLanguage(code: string): Promise<void> {
    await this.page.getByTestId("language-select").selectOption(code);
    await this.page
      .getByTestId("language-current")
      .filter({ hasText: code })
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async readLanguage(): Promise<string> {
    await this.page.getByTestId("language-current").waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("language-current").textContent()) ?? "";
  }

  // --- Devise de référence (REQ-CUR-001) ---

  async setReferenceCurrency(code: string): Promise<void> {
    await this.page.getByTestId("reference-currency-input").fill(code);
    await this.page.getByTestId("reference-currency-save").click();
    // Attend que l'affichage courant reflète la nouvelle devise.
    await this.page
      .getByTestId("reference-currency-current")
      .filter({ hasText: code })
      .waitFor({ state: "visible", timeout: 5000 });
  }

  async readReferenceCurrency(): Promise<string> {
    await this.page
      .getByTestId("reference-currency-current")
      .waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("reference-currency-current").textContent()) ?? "";
  }

  // --- Moyens de paiement (REQ-SUB-011) ---

  async createPaymentMethod(name: string): Promise<void> {
    await this.page.getByTestId("payment-method-new-name").fill(name);
    await this.page.getByTestId("payment-method-create").click();
  }

  async paymentMethodVisible(name: string): Promise<boolean> {
    try {
      await this.page
        .getByTestId("payment-method-name")
        .filter({ hasText: name })
        .first()
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async paymentMethodAbsent(name: string): Promise<boolean> {
    await this.page
      .getByTestId("payment-methods-list")
      .waitFor({ state: "visible", timeout: 5000 });
    return (
      (await this.page.getByTestId("payment-method-name").filter({ hasText: name }).count()) === 0
    );
  }

  // --- Agrégation multi-devises en mode dégradé (REQ-CUR-004) ---

  async computeAggregate(target: string, amounts: MoneyInput[]): Promise<void> {
    await this.page.getByTestId("exchange-target").fill(target);
    for (let i = 0; i < amounts.length; i++) {
      if (i > 0) {
        await this.page.getByTestId("exchange-add").click();
      }
      await this.page.getByTestId(`exchange-amount-${i}`).fill(amounts[i].amount);
      await this.page.getByTestId(`exchange-currency-${i}`).fill(amounts[i].currency);
    }
    await this.page.getByTestId("exchange-submit").click();
  }

  async aggregateIncompleteVisible(): Promise<boolean> {
    try {
      await this.page
        .getByTestId("exchange-incomplete")
        .waitFor({ state: "visible", timeout: 5000 });
      return true;
    } catch {
      return false;
    }
  }

  async readAggregateTotal(): Promise<string> {
    await this.page
      .getByTestId("exchange-total")
      .waitFor({ state: "visible", timeout: 5000 });
    return (await this.page.getByTestId("exchange-total").textContent()) ?? "";
  }
}
