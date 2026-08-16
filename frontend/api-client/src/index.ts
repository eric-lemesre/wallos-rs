/**
 * Contrat de l'API wallos-rs, **généré** depuis `api/openapi.json` — jamais écrit à la main.
 *
 * Paquet à part entière plutôt que dossier interne de l'interface (ADR 0057) : le contrat est
 * consommé par l'interface, mais aussi — à terme — par les coquilles, les tests et l'outillage.
 * En faire une dépendance déclarée leur évite d'en connaître le chemin sur disque, et rend
 * visible ce qu'ils dépendent réellement.
 *
 * La porte de non-dérive (R8) reste inchangée : `npm run ts-types-drift` régénère et refuse tout
 * écart avec le contrat committé.
 */
export type { paths, components, operations, webhooks } from "./schema";
