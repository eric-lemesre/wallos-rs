/**
 * Politique de mot de passe côté client (REQ-AUT-003). Le serveur reste l'autorité (validation
 * identique dans `core::password_policy`) ; cette copie donne un retour immédiat à l'utilisateur.
 *
 * @implements REQ-AUT-003
 */
export const MIN_PASSWORD_LENGTH = 12;

// Sous-ensemble représentatif de la liste serveur (entrées >= 12 caractères).
const COMPROMISED = new Set<string>([
  "password1234",
  "passwordpassword",
  "123456789012",
  "qwertyuiop123",
  "administrator",
  "adminadmin123",
  "letmeinplease",
  "welcome123456",
  "iloveyouforever",
  "superman12345",
]);

/** Indique si un mot de passe figure dans la liste de compromis embarquée. */
export function isCompromised(password: string): boolean {
  return COMPROMISED.has(password.toLowerCase());
}
