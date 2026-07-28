-- REQ-CUR-001 — devise de référence du foyer : la devise dans laquelle tous les agrégats sont
-- exprimés. Réglage porté par le foyer (entité de propriété, ADR 0012) ; défaut `EUR` (aligné sur
-- `ReferenceCurrency::DEFAULT_CODE`). N'altère jamais les montants saisis (conversion à l'affichage
-- seulement, REQ-CUR-004).
alter table households add column reference_currency text not null default 'EUR';
