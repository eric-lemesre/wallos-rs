import type { Page } from "@playwright/test";

import type { AppDriver, SignupInput } from "./AppDriver";

/**
 * Pilote `subtrack` via les `data-testid` stables (les sélecteurs CSS/XPath sont interdits, §7).
 */
export class TargetDriver implements AppDriver {
  constructor(
    private readonly page: Page,
    private readonly baseURL: string,
  ) {}

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
}
