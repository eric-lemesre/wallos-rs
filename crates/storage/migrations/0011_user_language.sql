-- REQ-I18N-001 — langue de l'utilisateur : choix personnel persisté côté serveur (conditionne aussi
-- les notifications émises par le serveur). Portée **utilisateur** (pas foyer) : chaque membre choisit
-- sa langue. `null` = non renseignée → l'UI applique la langue système si elle est supportée.
alter table users add column language text;
