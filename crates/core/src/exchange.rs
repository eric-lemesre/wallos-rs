//! Taux de change et conversion multi-devises (REQ-CUR-003).
//!
//! Contrat **pur** (ADR 0014) : ce module ne fait aucune I/O et ne connaît aucun réseau. Le trait
//! [`RateProvider`] est un **lookup** de taux ; le fetch HTTP (async) et la persistance vivent côté
//! serveur/stockage et alimentent une [`RateTable`] (l'adaptateur « dernier taux connu », toujours
//! disponible). La conversion conserve la **précision maximale** — l'arrondi n'intervient qu'au
//! formatage (REQ-CUR-005), jamais ici.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use wallos_req_macros::requirement;

use crate::DomainError;
use crate::money::{CurrencyCode, Money};

/// Taux de change d'une paire, daté et sourcé (traçabilité de l'origine).
///
/// `rate` exprime le nombre d'unités de `quote` pour **une** unité de `base`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeRate {
    base: CurrencyCode,
    quote: CurrencyCode,
    rate: Decimal,
    as_of: NaiveDate,
    source: String,
}

impl ExchangeRate {
    /// Construit un taux. Le taux doit être **strictement positif**.
    ///
    /// # Errors
    /// `DomainError::InvalidMoney` si `rate <= 0`.
    #[requirement(REQ-CUR-003)]
    pub fn new(
        base: CurrencyCode,
        quote: CurrencyCode,
        rate: Decimal,
        as_of: NaiveDate,
        source: impl Into<String>,
    ) -> Result<Self, DomainError> {
        if rate <= Decimal::ZERO {
            return Err(DomainError::InvalidMoney(format!(
                "non-positive exchange rate: {rate}"
            )));
        }
        Ok(Self {
            base,
            quote,
            rate,
            as_of,
            source: source.into(),
        })
    }

    /// Devise de base (dénominateur).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn base(&self) -> CurrencyCode {
        self.base
    }

    /// Devise cotée (numérateur).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn quote(&self) -> CurrencyCode {
        self.quote
    }

    /// Valeur du taux (`quote` par unité de `base`).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn rate(&self) -> Decimal {
        self.rate
    }

    /// Date de validité du taux.
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn as_of(&self) -> NaiveDate {
        self.as_of
    }

    /// Origine du taux (nom du fournisseur, « manual », …).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub fn source(&self) -> &str {
        &self.source
    }
}

/// Fournit le taux d'une paire de devises. Contrat **pur** (sans I/O), consommé par la conversion.
///
/// REQ-CUR-003 — la déclaration de méthode de trait ne peut porter `#[requirement]` (macro `fn` à
/// corps uniquement) ; les implémentations et les fonctions de conversion l'annotent.
pub trait RateProvider {
    /// Nombre d'unités de `quote` par unité de `base`, ou `None` si le taux est inconnu.
    /// La conversion d'une devise vers elle-même vaut toujours `1`.
    fn rate(&self, base: CurrencyCode, quote: CurrencyCode) -> Option<Decimal>;

    /// Taux **et sa date de validité** (fraîcheur, REQ-CUR-004) : `Some((taux, Some(date)))` pour un
    /// taux persisté, `Some((1, None))` pour une conversion identité (pas de date), `None` si inconnu.
    ///
    /// Défaut : reprend [`rate`](Self::rate) sans date (un fournisseur qui n'expose pas de date ne
    /// peut renseigner la fraîcheur). Les tables persistées le surchargent pour remonter `as_of`.
    /// (La définition par défaut de méthode de trait ne peut porter `#[requirement]` — macro `fn` à
    /// corps seul — ; les surcharges et les fonctions de conversion l'annotent.)
    fn dated_rate(
        &self,
        base: CurrencyCode,
        quote: CurrencyCode,
    ) -> Option<(Decimal, Option<NaiveDate>)> {
        self.rate(base, quote).map(|r| (r, None))
    }
}

/// Table de taux en mémoire — l'adaptateur « dernier taux connu » (ADR 0014), **toujours
/// disponible** et testable sans réseau. Alimentée par les taux persistés côté serveur.
#[derive(Debug, Clone, Default)]
pub struct RateTable {
    rates: Vec<ExchangeRate>,
}

impl RateTable {
    /// Construit une table à partir d'une collection de taux (dernier taux connu par paire).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub fn new(rates: Vec<ExchangeRate>) -> Self {
        Self { rates }
    }
}

impl RateProvider for RateTable {
    #[requirement(REQ-CUR-003)]
    fn rate(&self, base: CurrencyCode, quote: CurrencyCode) -> Option<Decimal> {
        if base == quote {
            return Some(Decimal::ONE);
        }
        self.rates
            .iter()
            .find(|r| r.base == base && r.quote == quote)
            .map(ExchangeRate::rate)
    }

    /// Remonte la date de validité du dernier taux connu (REQ-CUR-004) : identité → pas de date.
    #[requirement(REQ-CUR-004)]
    fn dated_rate(
        &self,
        base: CurrencyCode,
        quote: CurrencyCode,
    ) -> Option<(Decimal, Option<NaiveDate>)> {
        if base == quote {
            return Some((Decimal::ONE, None));
        }
        self.rates
            .iter()
            .find(|r| r.base == base && r.quote == quote)
            .map(|r| (r.rate, Some(r.as_of)))
    }
}

/// Convertit un montant vers `target` en précision maximale (aucun arrondi).
///
/// Renvoie `None` si aucun taux n'est connu pour la paire — l'appelant décide de l'exclusion et de
/// la signalisation (REQ-CUR-003/004), jamais une mise à zéro silencieuse.
#[requirement(REQ-CUR-003)]
pub fn convert(
    amount: &Money,
    target: CurrencyCode,
    provider: &impl RateProvider,
) -> Option<Money> {
    let rate = provider.rate(amount.currency(), target)?;
    // `Money::new` n'échoue pas : `amount` est positif ou nul et `rate` est positif.
    Money::new(amount.amount() * rate, target).ok()
}

/// Variante **datée** de [`convert`] (REQ-CUR-004) : renvoie aussi la date de validité du taux
/// utilisé (`None` pour une conversion identité). Sert à remonter la **fraîcheur** jusqu'à l'agrégat.
///
/// Renvoie `None` si aucun taux n'est connu pour la paire (mode dégradé : l'appelant exclut et
/// signale, jamais une mise à zéro silencieuse).
#[requirement(REQ-CUR-004)]
pub fn convert_dated(
    amount: &Money,
    target: CurrencyCode,
    provider: &impl RateProvider,
) -> Option<(Money, Option<NaiveDate>)> {
    let (rate, as_of) = provider.dated_rate(amount.currency(), target)?;
    // `Money::new` n'échoue pas : `amount` est positif ou nul et `rate` est positif.
    Money::new(amount.amount() * rate, target)
        .ok()
        .map(|m| (m, as_of))
}

/// Résultat d'une agrégation multi-devises convertie vers une devise cible.
///
/// `complete` est faux dès qu'un montant a été **exclu** faute de taux (signal de partialité,
/// REQ-CUR-003) : un écran de suivi doit l'indiquer, jamais afficher un total amputé en silence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertedTotal {
    total: Money,
    converted: usize,
    excluded: usize,
    complete: bool,
    as_of: Option<NaiveDate>,
}

impl ConvertedTotal {
    /// Total converti (dans la devise cible), en précision exacte.
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn total(&self) -> &Money {
        &self.total
    }

    /// Nombre de montants effectivement convertis et inclus.
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn converted(&self) -> usize {
        self.converted
    }

    /// Nombre de montants exclus faute de taux connu.
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn excluded(&self) -> usize {
        self.excluded
    }

    /// Vrai si tous les montants ont pu être convertis (agrégat complet).
    #[must_use]
    #[requirement(REQ-CUR-003)]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    /// Date de validité **la plus ancienne** parmi les taux réellement utilisés — la fraîcheur à
    /// afficher en mode dégradé (REQ-CUR-004). `None` si aucun taux daté n'a servi (ensemble vide,
    /// ou conversions en devise identique uniquement).
    #[must_use]
    #[requirement(REQ-CUR-004)]
    pub const fn as_of(&self) -> Option<NaiveDate> {
        self.as_of
    }
}

/// Agrège des montants multi-devises vers `target`, en **excluant** ceux sans taux connu et en
/// **signalant** la partialité (REQ-CUR-003).
///
/// Un ensemble vide, ou l'absence totale de taux, produit un total nul dans `target` marqué
/// `complete` selon qu'aucun montant n'a été exclu.
#[requirement(REQ-CUR-003)]
pub fn aggregate_converted(
    amounts: &[Money],
    target: CurrencyCode,
    provider: &impl RateProvider,
) -> ConvertedTotal {
    let mut total = Decimal::ZERO;
    let mut converted = 0usize;
    let mut excluded = 0usize;
    // Fraîcheur = date la plus ancienne (pire cas) parmi les taux datés effectivement utilisés
    // (REQ-CUR-004). Les conversions identité ne portent pas de date et ne l'affectent pas.
    let mut as_of: Option<NaiveDate> = None;
    for amount in amounts {
        match convert_dated(amount, target, provider) {
            Some((m, rate_as_of)) => {
                total += m.amount();
                converted += 1;
                if let Some(d) = rate_as_of {
                    as_of = Some(as_of.map_or(d, |cur| cur.min(d)));
                }
            }
            None => excluded += 1,
        }
    }
    ConvertedTotal {
        // `Money::new` n'échoue pas : `total` est une somme de montants positifs ou nuls.
        total: Money::new(total, target).unwrap_or_else(|_| Money::zero(target)),
        converted,
        excluded,
        complete: excluded == 0,
        as_of,
    }
}

#[cfg(test)]
mod tests {
    use wallos_req_macros::verifies;

    use super::*;

    fn cur(code: &str) -> CurrencyCode {
        CurrencyCode::new(code).unwrap()
    }

    fn dec(s: &str) -> Decimal {
        s.parse().unwrap()
    }

    fn money(units: &str, code: &str) -> Money {
        Money::new(dec(units), cur(code)).unwrap()
    }

    fn as_of() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 27).unwrap()
    }

    /// Table EUR->USD = 1.10, USD->GBP = 0.80.
    fn table() -> RateTable {
        RateTable::new(vec![
            ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.10"), as_of(), "test").unwrap(),
            ExchangeRate::new(cur("USD"), cur("GBP"), dec("0.80"), as_of(), "test").unwrap(),
        ])
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "accesseurs du taux")]
    fn exchange_rate_exposes_its_fields() {
        let r = ExchangeRate::new(cur("EUR"), cur("USD"), dec("1.10"), as_of(), "acme").unwrap();
        assert_eq!(r.base(), cur("EUR"));
        assert_eq!(r.quote(), cur("USD"));
        assert_eq!(r.rate(), dec("1.10"));
        assert_eq!(r.as_of(), as_of());
        assert_eq!(r.source(), "acme");
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "taux positif requis")]
    fn rejects_non_positive_rate() {
        assert!(ExchangeRate::new(cur("EUR"), cur("USD"), Decimal::ZERO, as_of(), "x").is_err());
        assert!(ExchangeRate::new(cur("EUR"), cur("USD"), dec("-1"), as_of(), "x").is_err());
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "devise identique = 1")]
    fn same_currency_is_identity() {
        let t = table();
        assert_eq!(t.rate(cur("EUR"), cur("EUR")), Some(Decimal::ONE));
        assert_eq!(
            convert(&money("42.50", "EUR"), cur("EUR"), &t),
            Some(money("42.50", "EUR"))
        );
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "conversion précision maximale")]
    fn converts_with_full_precision() {
        let t = table();
        // 10 EUR * 1.10 = 11.00 USD (précision conservée, pas d'arrondi).
        assert_eq!(
            convert(&money("10", "EUR"), cur("USD"), &t),
            Some(money("11.00", "USD"))
        );
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "paire inconnue -> None")]
    fn unknown_pair_is_none() {
        let t = table();
        assert_eq!(convert(&money("10", "GBP"), cur("EUR"), &t), None);
        assert_eq!(t.rate(cur("GBP"), cur("EUR")), None);
        // Base connue mais cotation absente : la paire complète doit correspondre (base ET quote),
        // jamais l'une OU l'autre — EUR est une base connue (EUR->USD) mais EUR->GBP est inconnu.
        assert_eq!(t.rate(cur("EUR"), cur("GBP")), None);
        // Symétrique : cotation connue (USD) mais base absente.
        assert_eq!(t.rate(cur("GBP"), cur("USD")), None);
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "agrégat complet")]
    fn aggregate_complete_when_all_convertible() {
        let t = table();
        let amounts = [money("10", "EUR"), money("5", "USD"), money("2", "USD")];
        let agg = aggregate_converted(&amounts, cur("USD"), &t);
        // 10 EUR -> 11 USD, + 5 + 2 = 18 USD.
        assert_eq!(agg.total(), &money("18.00", "USD"));
        assert_eq!(agg.converted(), 3);
        assert_eq!(agg.excluded(), 0);
        assert!(agg.is_complete());
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "agrégat partiel signalé")]
    fn aggregate_partial_excludes_and_flags() {
        let t = table();
        // GBP->USD inconnu : le montant GBP est exclu, l'agrégat est signalé incomplet.
        let amounts = [money("10", "EUR"), money("100", "GBP")];
        let agg = aggregate_converted(&amounts, cur("USD"), &t);
        assert_eq!(agg.total(), &money("11.00", "USD"));
        assert_eq!(agg.converted(), 1);
        assert_eq!(agg.excluded(), 1);
        assert!(!agg.is_complete());
    }

    #[test]
    #[verifies(REQ-CUR-003, case = "ensemble vide = neutre complet")]
    fn aggregate_empty_is_neutral_and_complete() {
        let agg = aggregate_converted(&[], cur("EUR"), &table());
        assert_eq!(agg.total(), &money("0", "EUR"));
        assert_eq!(agg.converted(), 0);
        assert_eq!(agg.excluded(), 0);
        assert!(agg.is_complete());
    }

    fn day(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// Table datée : EUR->USD au 20/07 (ancien), GBP->USD au 25/07 (récent).
    fn dated_table() -> RateTable {
        RateTable::new(vec![
            ExchangeRate::new(
                cur("EUR"),
                cur("USD"),
                dec("1.10"),
                day(2026, 7, 20),
                "acme",
            )
            .unwrap(),
            ExchangeRate::new(
                cur("GBP"),
                cur("USD"),
                dec("1.25"),
                day(2026, 7, 25),
                "acme",
            )
            .unwrap(),
        ])
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "fraîcheur = date la plus ancienne utilisée")]
    fn aggregate_surfaces_oldest_rate_date() {
        // Deux taux datés utilisés (20/07 et 25/07) : la fraîcheur remontée est la PLUS ANCIENNE.
        let agg = aggregate_converted(
            &[money("10", "EUR"), money("4", "GBP")],
            cur("USD"),
            &dated_table(),
        );
        // 10 * 1.10 + 4 * 1.25 = 11 + 5 = 16 USD.
        assert_eq!(agg.total(), &money("16.00", "USD"));
        assert!(agg.is_complete());
        assert_eq!(agg.as_of(), Some(day(2026, 7, 20)));
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "convert_dated remonte la date du taux")]
    fn convert_dated_carries_rate_date() {
        let t = dated_table();
        let (m, as_of) = convert_dated(&money("10", "EUR"), cur("USD"), &t).unwrap();
        assert_eq!(m, money("11.00", "USD"));
        assert_eq!(as_of, Some(day(2026, 7, 20)));
        // Paire inconnue : aucune conversion, aucune date.
        assert_eq!(convert_dated(&money("10", "JPY"), cur("USD"), &t), None);
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "identité : convertie, sans date de fraîcheur")]
    fn identity_conversion_has_no_staleness_date() {
        let t = dated_table();
        // Devise identique : taux 1, pas de taux daté -> as_of reste None mais l'agrégat est complet.
        let (m, as_of) = convert_dated(&money("42", "USD"), cur("USD"), &t).unwrap();
        assert_eq!(m, money("42", "USD"));
        assert_eq!(as_of, None);

        let agg = aggregate_converted(&[money("42", "USD")], cur("USD"), &t);
        assert_eq!(agg.total(), &money("42", "USD"));
        assert!(agg.is_complete());
        assert_eq!(agg.as_of(), None);
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "mélange daté/identité : date du seul taux daté")]
    fn mixed_dated_and_identity_uses_dated_date() {
        let t = dated_table();
        // Un montant USD (identité, sans date) + un montant EUR (taux daté 20/07).
        let agg = aggregate_converted(&[money("5", "USD"), money("10", "EUR")], cur("USD"), &t);
        // 5 (identité) + 11 (10 EUR * 1.10) = 16 USD.
        assert_eq!(agg.total(), &money("16.00", "USD"));
        assert!(agg.is_complete());
        assert_eq!(agg.as_of(), Some(day(2026, 7, 20)));
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "aucun taux : exclu et signalé, jamais zéro silencieux")]
    fn all_unconvertible_is_flagged_never_silent_zero() {
        // Table vide : aucun taux étranger connu (fournisseur indisponible / non configuré).
        let empty = RateTable::default();
        let agg = aggregate_converted(
            &[money("10", "EUR"), money("20", "GBP")],
            cur("USD"),
            &empty,
        );
        // Le total est nul EN CHIFFRES, mais l'agrégat est EXPLICITEMENT incomplet : jamais présenté
        // comme un zéro complet et silencieux (REQ-CUR-004, critère #2).
        assert_eq!(agg.total(), &money("0", "USD"));
        assert_eq!(agg.converted(), 0);
        assert_eq!(agg.excluded(), 2);
        assert!(!agg.is_complete());
        assert_eq!(agg.as_of(), None);
    }

    #[test]
    #[verifies(REQ-CUR-004, case = "dated_rate par défaut : taux sans date")]
    fn default_dated_rate_has_no_date() {
        // Un fournisseur qui n'implémente QUE `rate` hérite du défaut : taux repris, date absente.
        struct BareProvider;
        impl RateProvider for BareProvider {
            fn rate(&self, base: CurrencyCode, quote: CurrencyCode) -> Option<Decimal> {
                if base == cur("EUR") && quote == cur("USD") {
                    Some(dec("1.10"))
                } else {
                    None
                }
            }
        }
        let p = BareProvider;
        assert_eq!(
            p.dated_rate(cur("EUR"), cur("USD")),
            Some((dec("1.10"), None))
        );
        assert_eq!(p.dated_rate(cur("GBP"), cur("USD")), None);
        // L'agrégat reste complet (taux connu) mais sans date de fraîcheur remontée.
        let agg = aggregate_converted(&[money("10", "EUR")], cur("USD"), &p);
        assert_eq!(agg.total(), &money("11.00", "USD"));
        assert!(agg.is_complete());
        assert_eq!(agg.as_of(), None);
    }
}
