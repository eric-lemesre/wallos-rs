-- REQ-CAT-004 — unicité du nom de catégorie **par foyer**, insensible à la casse (deux catégories
-- homonymes rendraient les statistiques illisibles). L'index porte sur `(household_id, lower(name))` :
-- « Streaming » et « streaming » entrent en collision dans un même foyer, mais un autre foyer peut avoir
-- sa propre catégorie « Streaming » (isolation §9). Aligné sur `Category::canonical_name`.
create unique index categories_household_name_unique on categories (household_id, lower(name));
