import createClient from "openapi-fetch";
import type { components, paths } from "./schema";

/**
 * Client HTTP typé de l'API wallos-rs, dérivé exclusivement du contrat OpenAPI
 * généré (`api/openapi.json`). Aucun type d'entité n'est écrit à la main.
 *
 * @implements REQ-OPS-001
 */
export const api = createClient<paths>({ baseUrl: "/api/v1" });

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

export type { paths, components } from "./schema";
