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
  logout(): Promise<void>;
  currentUserGone(): Promise<boolean>;

  // Gestion des appareils (REQ-AUT-006).
  /** Appaire un appareil via l'API (comme le ferait une coquille native). */
  pairDevice(input: SignupInput, label: string, platform: string): Promise<void>;
  /** Recharge la vue des appareils (la liste se rafraîchit avec la session courante). */
  openDevices(): Promise<void>;
  deviceListed(label: string): Promise<boolean>;
  revokeDevice(label: string): Promise<void>;
  deviceGone(label: string): Promise<boolean>;

  // Changement de mot de passe (REQ-AUT-007).
  changePassword(current: string, next: string): Promise<void>;
  passwordChangeSucceeded(): Promise<boolean>;

  // Catégories (REQ-CAT-001).
  /** Crée une catégorie via le formulaire. */
  createCategory(name: string): Promise<void>;
  /** Vrai si une catégorie de ce nom est visible dans la liste. */
  categoryVisible(name: string): Promise<boolean>;
  /** Vrai si aucune catégorie de ce nom n'apparaît (isolation / suppression). */
  categoryAbsent(name: string): Promise<boolean>;

  // Calcul d'échéance (REQ-SUB-012).
  /** Renseigne ancre/unité/intervalle/date de référence et déclenche le calcul. */
  computeNextDue(anchor: string, unit: string, interval: string, after: string): Promise<void>;
  /** Texte de la prochaine échéance affichée. */
  readNextDue(): Promise<string>;

  // Agrégation multi-devises en mode dégradé (REQ-CUR-004).
  /** Saisit une devise cible et des montants, puis déclenche le calcul de l'agrégat. */
  computeAggregate(target: string, amounts: MoneyInput[]): Promise<void>;
  /** Vrai si l'agrégat est explicitement signalé incomplet (montants exclus faute de taux). */
  aggregateIncompleteVisible(): Promise<boolean>;
  /** Texte du total affiché (jamais un zéro silencieux : la part convertible reste visible). */
  readAggregateTotal(): Promise<string>;
}

export interface MoneyInput {
  amount: string;
  currency: string;
}
