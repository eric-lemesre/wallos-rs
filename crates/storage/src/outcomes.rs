//! Issues communes des insertions d'entités possédées (REQ-SYN-001 / revue F1-F2).
//!
//! Une création peut échouer sur une contrainte d'unicité de deux natures **distinctes** :
//! l'identifiant (clé primaire `id`, potentiellement **fourni par le client**, REQ-SYN-001) ou une
//! règle métier (p. ex. l'unicité de nom par foyer, REQ-CAT-004). Les distinguer permet à l'API de
//! renvoyer un statut correct — `409` pour un `id` déjà pris, `422` pour un doublon métier — au lieu
//! d'un `500` ou d'un message mensonger.

/// Résultat d'une création d'entité possédée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateOutcome {
    /// L'entité a été créée.
    Created,
    /// L'`id` fourni est **déjà pris** (collision de clé primaire) → l'appelant renvoie `409`.
    DuplicateId,
    /// Une règle d'unicité **métier** est violée (nom déjà utilisé dans le foyer) → `422`.
    DuplicateName,
}
