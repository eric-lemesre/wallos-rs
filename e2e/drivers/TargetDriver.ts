import type { Page } from "@playwright/test";

import type { AppDriver, SignupInput } from "./AppDriver";
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
}
