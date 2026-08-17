import type { Page } from "@playwright/test";

import { LegacyDriver } from "./LegacyDriver";
import { TargetDriver } from "./TargetDriver";

/** Identifiants d'un compte (agnostiques de la cible). */
export interface Credentials {
  email: string;
  password: string;
}

/** Abonnement décrit de façon agnostique : chaque pilote le traduit dans les termes de sa cible. */
export interface SubscriptionInput {
  name: string;
  /** Montant en unités majeures, séparateur décimal `.` (ex. « 120.00 »). */
  amount: string;
  /** Code ISO 4217 (ex. « EUR »). */
  currency: string;
  unit: "day" | "week" | "month" | "year";
  /** Nombre d'unités par cycle (ex. « 2 » pour « tous les deux ans »). */
  interval: string;
  /** Première échéance, au format `YYYY-MM-DD`. */
  firstPayment: string;
}

/**
 * Contrat **agnostique de l'implémentation**, partagé par les deux cibles (AGENTS.md §8.1).
 *
 * Il s'étoffe au fur et à mesure des exigences de parité (R10/R11) : ce que l'on ajoute ici doit
 * pouvoir être exprimé **des deux côtés**, faute de quoi le scénario cesserait d'être agnostique et
 * ne prouverait plus rien.
 */
export interface Harness {
  /** Crée un compte si nécessaire puis ouvre une session. */
  signIn(user: Credentials): Promise<void>;
  /** Vrai si une session est active (accès au tableau de bord). */
  signedIn(): Promise<boolean>;

  /** Crée un abonnement. Les valeurs par défaut de la cible s'appliquent aux champs non fournis. */
  createSubscription(input: SubscriptionInput): Promise<void>;

  /**
   * Coût mensuel **normalisé** de l'ensemble des abonnements, rendu canonique : séparateur `.`,
   * sans symbole ni séparateur de milliers (ex. « 10.00 »).
   *
   * La normalisation appartient au PILOTE, non au scénario : Wallos rend « €10.00 Monthly Cost »
   * là où subtrack rend un montant localisé. Un scénario qui découperait ces chaînes cesserait
   * d'être agnostique.
   */
  readMonthlyTotal(): Promise<string>;
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
