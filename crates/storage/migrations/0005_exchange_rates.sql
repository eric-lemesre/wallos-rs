-- REQ-CUR-003 — taux de change persistés avec leur date de validité et leur source.
-- Donnée de RÉFÉRENCE globale (marché), pas une entité de compte : pas d'owner_id/household_id
-- (l'isolation §9 protège les données DE COMPTE ; les taux sont partagés, comme le référentiel de
-- devises). `rate` en `numeric` (jamais un flottant, R4). `fetched_at` est fourni par l'appelant
-- (instant injecté), `as_of` est la date de validité du taux.

create table exchange_rates (
    base_currency  text        not null,
    quote_currency text        not null,
    rate           numeric     not null,
    as_of          date        not null,
    source         text        not null,
    fetched_at     timestamptz not null,
    primary key (base_currency, quote_currency, as_of)
);

create index exchange_rates_pair_idx on exchange_rates (base_currency, quote_currency, as_of desc);
