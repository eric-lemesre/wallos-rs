import type { Page } from "@playwright/test";

import { LegacyDriver } from "./LegacyDriver";
import { TargetDriver } from "./TargetDriver";

/** Identifiants d'un compte (agnostiques de la cible). */
export interface Credentials {
  email: string;
  password: string;
}

/**
 * Contrat **agnostique de l'implémentation** minimal, partagé par les deux cibles (AGENTS.md §8.1).
 * Il s'étoffera (opérations métier : abonnements, devises, catégories) avec la première exigence
 * `oracle: legacy` ; pour l'instant il couvre l'authentification, seul comportement commun à
 * subtrack et à Wallos.
 */
export interface Harness {
  /** Crée un compte si nécessaire puis ouvre une session. */
  signIn(user: Credentials): Promise<void>;
  /** Vrai si une session est active (accès au tableau de bord). */
  signedIn(): Promise<boolean>;
}

/** Cible d'exécution d'un scénario (§8.1). */
export type Target = "app" | "legacy";

/**
 * Fabrique de driver : `TARGET=legacy` pilote Wallos (app d'origine), sinon subtrack.
 * Un même scénario s'exécute ainsi contre les deux cibles sans duplication.
 */
export function createHarness(
  page: Page,
  baseURL: string,
  target: Target = (process.env.TARGET as Target) ?? "app",
): Harness {
  return target === "legacy"
    ? new LegacyDriver(page, baseURL)
    : new TargetDriver(page, baseURL);
}
