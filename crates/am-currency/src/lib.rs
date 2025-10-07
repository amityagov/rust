#[cfg(feature = "sqlx")]
mod sqlx_support;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use strum::IntoEnumIterator;
use strum_macros::{Display, EnumIter, EnumString};

pub type FiatCurrency = iso_currency::Currency;

#[derive(Clone, Debug, Display, EnumString, EnumIter, Serialize, Deserialize)]
#[strum(serialize_all = "UPPERCASE")]
pub enum CryptoCurrency {
    #[strum(to_string = "USDC_SOL")]
    UsdcSol,

    #[strum(to_string = "BTC")]
    Btc,
}

#[derive(Debug, Clone)]
pub enum Currency {
    Fiat(FiatCurrency),
    Crypto(CryptoCurrency),
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Currency::Fiat(fiat) => write!(f, "{}", fiat.code()),
            Currency::Crypto(crypto) => write!(f, "{crypto}"),
        }
    }
}
impl Currency {
    pub fn all() -> impl Iterator<Item = Currency> {
        let fiat = FiatCurrency::iter().map(Currency::Fiat);
        let crypto = CryptoCurrency::iter().map(Currency::Crypto);
        fiat.chain(crypto)
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

    pub fn fiat(iso_currency: FiatCurrency) -> Self {
        Currency::Fiat(iso_currency)
    }

    pub fn crypto(crypto_currency: CryptoCurrency) -> Self {
        Currency::Crypto(crypto_currency)
    }
}

impl From<FiatCurrency> for Currency {
    fn from(value: FiatCurrency) -> Self {
        Currency::Fiat(value)
    }
}

impl From<CryptoCurrency> for Currency {
    fn from(value: CryptoCurrency) -> Self {
        Currency::Crypto(value)
    }
}

impl Serialize for Currency {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Currency::Fiat(fiat) => serializer.serialize_str(fiat.code()),
            Currency::Crypto(crypto) => serializer.serialize_str(&crypto.to_string()),
        }
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;

        // Try to parse as fiat
        if let Ok(cur) = FiatCurrency::from_str(&s) {
            return Ok(Currency::Fiat(cur));
        }

        // Try to parse as crypto
        if let Ok(crypto) = CryptoCurrency::from_str(&s) {
            return Ok(Currency::Crypto(crypto));
        }

        Err(serde::de::Error::custom(format!("Unknown currency: {s}")))
    }
}
