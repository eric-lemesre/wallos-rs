/**
 * Substitut de logo **déterministe, généré localement** (REQ-SUB-015).
 *
 * Quand un abonnement n'a pas de logo, on affiche un substitut dérivé **uniquement** de son nom :
 * des initiales sur une couleur stable. Fonction **pure** : aucune requête réseau, aucun appel à un
 * service tiers (acceptance #1/#2 — la confidentialité est préservée par conception, jamais de fetch
 * automatique de logo). Le même nom produit toujours le même substitut (idempotent).
 */
export interface LogoSubstitute {
  /** 1 à 2 initiales en majuscules dérivées du nom (`?` si le nom est vide). */
  initials: string;
  /** Couleur de fond stable dérivée du nom (chaîne CSS `hsl(...)`). */
  color: string;
}

/** Calcule le substitut de logo déterministe d'un nom d'abonnement. */
export function logoSubstitute(name: string): LogoSubstitute {
  const trimmed = name.trim();
  const words = trimmed.split(/\s+/).filter((w) => w.length > 0);
  const initials =
    words.length === 0
      ? "?"
      : words
          .slice(0, 2)
          // Revue SUB-015 #1 : première **unité de code** (pas unité UTF-16) — préserve emoji / hors-BMP.
          .map((w) => String.fromCodePoint(w.codePointAt(0)!).toUpperCase())
          .join("");

  // Teinte dérivée d'un hachage déterministe du nom (aucune source d'aléa, aucun réseau).
  let hash = 0;
  for (let i = 0; i < trimmed.length; i += 1) {
    hash = (hash * 31 + trimmed.charCodeAt(i)) >>> 0;
  }
  const hue = hash % 360;
  // L=40 % pour un contraste suffisant avec un texte blanc (revue SUB-015 #6).
  return { initials, color: `hsl(${hue}, 65%, 40%)` };
}
