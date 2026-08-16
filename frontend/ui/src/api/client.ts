import createClient from "openapi-fetch";
import type { components, paths } from "@wallos/api-client";

/** Chemin du contrat, invariant : seule l'**origine** qui le précède dépend de la coquille. */
const API_PATH = "/api/v1";

/**
 * Client HTTP typé de l'API wallos-rs, dérivé exclusivement du contrat OpenAPI
 * généré (`api/openapi.json`). Aucun type d'entité n'est écrit à la main.
 *
 * @implements REQ-OPS-001
 */
export const api = createClient<paths>({ baseUrl: API_PATH });

/** Origine déjà appliquée, pour rendre `configureApi` idempotente. `null` = jamais configurée. */
let origineAppliquee: string | null = null;

/**
 * Fixe l'origine de l'API pour la durée d'exécution de l'application.
 *
 * Une coquille **web** ne fournit rien : l'API est servie sur la même origine (REQ-OPS-003) et
 * l'URL relative convient. Une coquille **native** n'a aucune origine implicite et doit la
 * désigner (REQ-CLT-005). Appelée par `App`, jamais par un composant.
 *
 * **Idempotente** : rappelée avec la même origine, elle ne recrée rien — `App` peut donc l'appeler
 * à chaque rendu sans invalider le client en cours d'utilisation.
 *
 * Les méthodes du nouveau client sont recopiées **sur l'objet existant** plutôt que de le
 * remplacer : les composants ont capturé `api` à l'import, et une réaffectation leur laisserait
 * l'ancienne origine. Cela préserve du même coup l'objet que les tests instrumentent.
 *
 * @implements REQ-CLT-003
 */
export function configureApi(apiBaseUrl: string) {
  const origine = apiBaseUrl.replace(/\/+$/, "");
  if (origine === origineAppliquee) {
    return;
  }
  origineAppliquee = origine;
  Object.assign(api, createClient<paths>({ baseUrl: `${origine}${API_PATH}` }));
}

/** Réponse de l'endpoint de santé, issue du contrat. */
export type HealthResponse = components["schemas"]["HealthResponse"];

/**
 * Erreur RFC 9457 (`application/problem+json`), schéma d'erreur unique de l'API.
 *
 * @implements REQ-SEC-002
 */
export type Problem = components["schemas"]["Problem"];

/**
 * Corps de la requête de création de compte, issu du contrat.
 *
 * @implements REQ-AUT-001
 */
export type CreateAccountRequest = components["schemas"]["CreateAccountRequest"];

/**
 * Corps de la requête d'authentification, issu du contrat.
 *
 * @implements REQ-AUT-002
 */
export type CreateSessionRequest = components["schemas"]["CreateSessionRequest"];

/**
 * Compte authentifié courant, issu du contrat.
 *
 * @implements REQ-AUT-002
 */
export type CurrentUser = components["schemas"]["CurrentUser"];

export type { paths, components } from "@wallos/api-client";
