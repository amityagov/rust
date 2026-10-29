//! Fiat and crypto currencies behind one type, with a single textual form.
//!
//! A [`Currency`] is written as its code everywhere: serde, [`Display`](std::fmt::Display),
//! [`FromStr`] and, with the `sqlx` feature, database columns. Fiat currencies use their ISO 4217
//! code (`USD`), crypto currencies the code of [`CryptoCurrency`] (`BTC`, `USDC_SOL`).

#[cfg(feature = "sqlx")]
mod sqlx;

use std::fmt::{self, Display, Formatter};
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

/// An ISO 4217 currency.
pub type FiatCurrency = iso_currency::Currency;

/// The supported crypto currencies, identified by their code.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Hash, Display, EnumString, EnumIter, Serialize, Deserialize,
)]
pub enum CryptoCurrency {
    /// USDC on Solana.
    #[strum(to_string = "USDC_SOL")]
    #[serde(rename = "USDC_SOL")]
    UsdcSol,

    /// Bitcoin.
    #[strum(to_string = "BTC")]
    #[serde(rename = "BTC")]
    Btc,

    /// Ether.
    #[strum(to_string = "ETH")]
    #[serde(rename = "ETH")]
    Eth,
}

/// A fiat or a crypto currency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Currency {
    Fiat(FiatCurrency),
    Crypto(CryptoCurrency),
}

/// The text is not the code of a known fiat or crypto currency.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("unknown currency: {0}")]
pub struct UnknownCurrency(pub String);

impl Currency {
    /// Every fiat currency followed by every crypto currency.
    pub fn all() -> impl Iterator<Item = Currency> {
        Self::all_fiat().chain(Self::all_crypto())
    }

    pub fn all_fiat() -> impl Iterator<Item = Currency> {
        FiatCurrency::iter().map(Currency::Fiat)
    }

    pub fn all_crypto() -> impl Iterator<Item = Currency> {
        CryptoCurrency::iter().map(Currency::Crypto)
    }

    pub fn is_fiat(&self) -> bool {
        matches!(self, Currency::Fiat(_))
    }

    pub fn is_crypto(&self) -> bool {
        matches!(self, Currency::Crypto(_))
    }

    pub fn fiat(fiat: FiatCurrency) -> Self {
        Currency::Fiat(fiat)
    }

    pub fn crypto(crypto: CryptoCurrency) -> Self {
        Currency::Crypto(crypto)
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Currency::Fiat(fiat) => f.write_str(fiat.code()),
            Currency::Crypto(crypto) => Display::fmt(crypto, f),
        }
    }
}

impl FromStr for Currency {
    type Err = UnknownCurrency;

    /// Parses a fiat code first, then a crypto code.
    fn from_str(code: &str) -> Result<Self, Self::Err> {
        FiatCurrency::from_str(code)
            .map(Currency::Fiat)
            .or_else(|_| CryptoCurrency::from_str(code).map(Currency::Crypto))
            .map_err(|_| UnknownCurrency(code.to_owned()))
    }
}

impl From<FiatCurrency> for Currency {
    fn from(fiat: FiatCurrency) -> Self {
        Currency::Fiat(fiat)
    }
}

impl From<CryptoCurrency> for Currency {
    fn from(crypto: CryptoCurrency) -> Self {
        Currency::Crypto(crypto)
    }
}

impl Serialize for Currency {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_through_text() {
        for currency in Currency::all() {
            assert_eq!(currency.to_string().parse::<Currency>(), Ok(currency));
        }
    }

    #[test]
    fn unknown_code_is_rejected() {
        assert_eq!(
            "NOPE".parse::<Currency>(),
            Err(UnknownCurrency("NOPE".to_owned()))
        );
    }

    #[test]
    fn serde_uses_the_code_for_both_kinds() {
        let usd = serde_json::to_string(&Currency::fiat(FiatCurrency::USD)).unwrap();
        let usdc = serde_json::to_string(&Currency::crypto(CryptoCurrency::UsdcSol)).unwrap();
        assert_eq!((usd.as_str(), usdc.as_str()), (r#""USD""#, r#""USDC_SOL""#));
        assert_eq!(
            serde_json::from_str::<Currency>(&usdc).unwrap(),
            Currency::Crypto(CryptoCurrency::UsdcSol)
        );
    }

    #[test]
    fn crypto_currency_serializes_to_its_display_form() {
        for crypto in CryptoCurrency::iter() {
            assert_eq!(
                serde_json::to_string(&crypto).unwrap(),
                format!("\"{crypto}\"")
            );
        }
    }
}
