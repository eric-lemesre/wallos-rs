# ADR 0005 — Suppression du `expect` dans `CurrencyCode::as_str`

## Contexte

`CurrencyCode::as_str` utilisait `.expect("currency code is ASCII")`. Le constructeur garantit que les octets sont ASCII alphabétiques, donc `from_utf8` est infaillible. Cependant R5 interdit `expect` hors test/main. La conversion peut être réécrite sans `expect`.

## Décision

Remplacer `from_utf8(...).expect(...)` par une conversion `const` ou par `unsafe` justifié avec `from_utf8_unchecked`, documenté. Le choix retenu est `unsafe` encapsulé, avec un commentaire de justification, car il s'agit d'un invariant maintenu par le constructeur.

## Conséquences

- R5 respecté dans `core`.
- Un test unitaire supplémentaire exerce `as_str`.

## Liens

- AGENTS.md §0 (R5).
- REQ-SUB-001 (modèle abonnement), REQ-CUR-002 (représentation monétaire).
