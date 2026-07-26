/**
 * Interface agnostique de l'implémentation (AGENTS.md §8.1). Les scénarios sont écrits une fois
 * contre cette interface et exécutés contre `TargetDriver` (subtrack). Un `LegacyDriver` sera
 * ajouté avec la première exigence `oracle: legacy`.
 */
export interface SignupInput {
  email: string;
  password: string;
}

export interface AppDriver {
  gotoSignup(): Promise<void>;
  signup(input: SignupInput): Promise<void>;
  signupSucceeded(): Promise<boolean>;
  hasPasswordError(): Promise<boolean>;
  login(input: SignupInput): Promise<void>;
  currentUserVisible(): Promise<boolean>;
  loginFailed(): Promise<boolean>;
}
