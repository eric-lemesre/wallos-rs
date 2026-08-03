import type { ParseKeys } from "i18next";

/**
 * Identité typée sur une clé de traduction (REQ-I18N-002, critère 2).
 *
 * Sert à annoter les littéraux passés hors de `t(...)` — typiquement les messages de validation des
 * schémas zod — pour que `tsc` vérifie qu'ils appartiennent au catalogue de référence. La valeur
 * retournée reste une chaîne, acceptée telle quelle par les API tierces (zod, react-hook-form).
 *
 * @implements REQ-I18N-002
 */
export const tKey = (key: ParseKeys): ParseKeys => key;
