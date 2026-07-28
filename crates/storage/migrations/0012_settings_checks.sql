-- REQ-CUR-001 / I18N-001 — durcissement d'intégrité (suite revue CUR-001). Le domaine valide déjà
-- (ReferenceCurrency::parse / Language::parse) mais la base ne le garantissait pas : une valeur corrompue
-- (accès direct) fausserait la lecture. On borne structurellement les réglages aux formes valides.
alter table households
    add constraint households_reference_currency_iso4217 check (reference_currency ~ '^[A-Z]{3}$');

alter table users
    add constraint users_language_supported check (language is null or language in ('en', 'fr'));
