// Normalisation des montants rendus. Module SÉPARÉ du contrat de pilotage : `Harness` importe les
// deux pilotes, et les pilotes ont besoin de cette fonction — la placer ici évite un cycle
// d'imports à l'exécution. Il se résolvait, mais dépendre de l'ordre d'évaluation des modules pour
// une fonction utilitaire est une fragilité gratuite.

/**
 * Ramène un montant rendu à sa forme canonique.
 *
 * Accepte les deux conventions décimales : « 1 234,56 » (virgule) comme « 1,234.56 » (point). Le
 * dernier séparateur rencontré est le séparateur décimal — heuristique suffisante ici, où les
 * montants viennent d'un rendu d'interface et non d'une saisie libre.
 */
export function canonicalAmount(rendu: string): string {
  const chiffres = rendu.replace(/[^0-9.,]/g, "");
  const dernier = Math.max(chiffres.lastIndexOf("."), chiffres.lastIndexOf(","));
  if (dernier === -1) {
    return `${Number(chiffres).toFixed(2)}`;
  }
  const entier = chiffres.slice(0, dernier).replace(/[.,]/g, "");
  const decimales = chiffres.slice(dernier + 1);
  return Number(`${entier}.${decimales}`).toFixed(2);
}
