import { logoSubstitute } from "../lib/logoSubstitute";

/**
 * Logo d'un abonnement (REQ-SUB-015). Affiche le logo fourni s'il existe, sinon un **substitut
 * déterministe généré localement** (initiales + couleur dérivées du nom) — **jamais de requête réseau**
 * ni de récupération automatique auprès d'un service tiers (choix de confidentialité par conception).
 *
 * @implements REQ-SUB-015
 */
export function SubscriptionLogo({ name, logo }: { name: string; logo?: string | null }) {
  // Revue SUB-015 #2 : un logo « blanc » (espaces) n'est pas un logo -> on affiche le substitut.
  if (logo?.trim()) {
    return <img data-testid="subscription-logo-image" src={logo} alt={name} width={32} height={32} />;
  }
  const { initials, color } = logoSubstitute(name);
  return (
    <span
      data-testid="subscription-logo-substitute"
      aria-hidden="true"
      style={{
        backgroundColor: color,
        color: "var(--text-color-inverted)",
        width: 32,
        height: 32,
        display: "inline-flex",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      {initials}
    </span>
  );
}
