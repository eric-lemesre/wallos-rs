/**
 * Formatage localisé des montants (REQ-CUR-006) et des dates (REQ-I18N-003).
 *
 * La locale d'affichage dérive de la **langue de l'interface** (i18next, elle-même pilotée par le
 * réglage du compte — REQ-I18N-001) : `fr` applique les conventions françaises, `en` les
 * anglaises. Les données du serveur restent des chaînes canoniques (montant décimal R4, dates
 * `YYYY-MM-DD`) ; seule la PRÉSENTATION est localisée, jamais la valeur (ADR 0051).
 *
 * @implements REQ-CUR-006
 * @implements REQ-I18N-003
 */

/**
 * Montant localisé avec symbole et décimales de la devise (REQ-CUR-006). `Intl.NumberFormat`
 * porte les données CLDR/ISO 4217 : séparateurs et position du symbole suivent la locale, le
 * nombre de décimales suit la devise (une devise sans sous-unité — JPY — n'affiche aucune
 * décimale, critère #2). Une devise inconnue d'Intl ou un montant non numérique retombent sur
 * la forme brute `montant CODE` (jamais de crash d'affichage).
 */
export function formatAmount(amount: string, currency: string, locale: string): string {
  const trimmed = amount.trim();
  if (trimmed === "") {
    // Champ absent : rien à afficher (revue CUR-006 F8 — ni « €0.00 », ni un code orphelin).
    return "";
  }
  // Seule la forme décimale canonique R4 est formatée (revue F7 : pas de `0x10`/`1e2` silencieux),
  // et seulement si elle tient sans perte dans un double (revue F6 : ≤ 15 chiffres significatifs —
  // au-delà, la forme brute préserve la valeur exacte).
  const canonical = /^-?\d+(\.\d+)?$/.test(trimmed);
  const digits = trimmed.replace(/[^0-9]/g, "").replace(/^0+/, "").length;
  const value = Number(trimmed);
  if (!canonical || digits > 15 || !Number.isFinite(value)) {
    return `${amount} ${currency}`.trim();
  }
  try {
    return new Intl.NumberFormat(locale, { style: "currency", currency }).format(value);
  } catch {
    return `${amount} ${currency}`;
  }
}

/**
 * Date civile `YYYY-MM-DD` localisée (REQ-I18N-003). La chaîne est décomposée puis reconstruite
 * en date **locale** : passer l'ISO brut à `new Date(...)` l'interpréterait en UTC minuit, et
 * tout fuseau à l'ouest de Greenwich afficherait LA VEILLE (critère #2 « sans décalage d'un
 * jour »). Chaîne non conforme → renvoyée telle quelle.
 */
export function formatDate(iso: string, locale: string): string {
  const parts = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
  if (!parts) {
    return iso;
  }
  const civil = new Date(Number(parts[1]), Number(parts[2]) - 1, Number(parts[3]));
  try {
    return new Intl.DateTimeFormat(locale, { dateStyle: "medium" }).format(civil);
  } catch {
    return iso;
  }
}

/**
 * Mois civil `YYYY-MM` localisé (REQ-I18N-003, évolution des coûts). Même garde de fuseau que
 * [`formatDate`].
 */
export function formatMonth(yearMonth: string, locale: string): string {
  const parts = /^(\d{4})-(\d{2})$/.exec(yearMonth);
  if (!parts) {
    return yearMonth;
  }
  const civil = new Date(Number(parts[1]), Number(parts[2]) - 1, 1);
  try {
    return new Intl.DateTimeFormat(locale, { year: "numeric", month: "short" }).format(civil);
  } catch {
    return yearMonth;
  }
}
