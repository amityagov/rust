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

    /// Solana.
    #[strum(to_string = "SOL")]
    #[serde(rename = "SOL")]
    Sol,

    /// Solana devnet.
    #[strum(to_string = "SOL_TEST")]
    #[serde(rename = "SOL_TEST")]
    SolTest,

    /// Toncoin.
    #[strum(to_string = "TON")]
    #[serde(rename = "TON")]
    Ton,

    /// Toncoin on the TON testnet.
    #[strum(to_string = "TON_TEST")]
    #[serde(rename = "TON_TEST")]
    TonTest,

    /// Bitcoin testnet.
    #[strum(to_string = "BTC_TEST")]
    #[serde(rename = "BTC_TEST")]
    BtcTest,

    /// Ether on the Sepolia testnet.
    #[strum(to_string = "ETH_TEST")]
    #[serde(rename = "ETH_TEST")]
    EthTest,

    /// USDC on Ethereum.
    #[strum(to_string = "USDC_ERC20")]
    #[serde(rename = "USDC_ERC20")]
    UsdcErc20,

    /// USDT on Ethereum.
    #[strum(to_string = "USDT_ERC20")]
    #[serde(rename = "USDT_ERC20")]
    UsdtErc20,

    /// USDT on Tron.
    #[strum(to_string = "USDT_TRC20")]
    #[serde(rename = "USDT_TRC20")]
    UsdtTrc20,

    /// USDC on the Solana devnet.
    #[strum(to_string = "USDC_SOL_TEST")]
    #[serde(rename = "USDC_SOL_TEST")]
    UsdcSolTest,

    /// USDT on the Solana devnet.
    #[strum(to_string = "USDT_SOL_TEST")]
    #[serde(rename = "USDT_SOL_TEST")]
    UsdtSolTest,

    /// USDT on the Ethereum Sepolia testnet.
    #[strum(to_string = "USDT_ETH_TEST")]
    #[serde(rename = "USDT_ETH_TEST")]
    UsdtEthTest,

    /// ALTXUSDT token on the TON testnet.
    #[strum(to_string = "ALTXUSDT_TEST")]
    #[serde(rename = "ALTXUSDT_TEST")]
    AltxusdtTest,
}

impl CryptoCurrency {
    /// Decimal places of the smallest unit (`1 SOL` is `10^9` lamports).
    pub fn decimals(&self) -> u32 {
        match self {
            CryptoCurrency::Btc | CryptoCurrency::BtcTest => 8,
            CryptoCurrency::Eth | CryptoCurrency::EthTest => 18,
            CryptoCurrency::Sol
            | CryptoCurrency::SolTest
            | CryptoCurrency::Ton
            | CryptoCurrency::TonTest => 9,
            CryptoCurrency::UsdcSol
            | CryptoCurrency::UsdcSolTest
            | CryptoCurrency::UsdcErc20
            | CryptoCurrency::UsdtErc20
            | CryptoCurrency::UsdtTrc20
            | CryptoCurrency::UsdtSolTest
            | CryptoCurrency::UsdtEthTest
            | CryptoCurrency::AltxusdtTest => 6,
        }
    }
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

    /// Decimal places of the smallest unit; `None` for a fiat currency that has no minor unit.
    pub fn decimals(&self) -> Option<u32> {
        match self {
            Currency::Fiat(fiat) => fiat.exponent().map(u32::from),
            Currency::Crypto(crypto) => Some(crypto.decimals()),
        }
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
    fn the_crypto_list_is_the_list_of_every_system_asset() {
        // The codes are the `asset_id` of the system assets of Custody and the codes of the
        // Ledger currencies; a code added or renamed here is a decision for all of them.
        let codes: Vec<String> = CryptoCurrency::iter().map(|c| c.to_string()).collect();
        assert_eq!(
            codes,
            [
                "USDC_SOL",
                "BTC",
                "ETH",
                "SOL",
                "SOL_TEST",
                "TON",
                "TON_TEST",
                "BTC_TEST",
                "ETH_TEST",
                "USDC_ERC20",
                "USDT_ERC20",
                "USDT_TRC20",
                "USDC_SOL_TEST",
                "USDT_SOL_TEST",
                "USDT_ETH_TEST",
                "ALTXUSDT_TEST",
            ]
        );
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), codes.len());
    }

    #[test]
    fn crypto_decimals_are_those_of_the_networks() {
        let decimals: Vec<(String, u32)> = CryptoCurrency::iter()
            .map(|c| (c.to_string(), c.decimals()))
            .collect();
        for (code, expected) in [
            ("BTC", 8),
            ("BTC_TEST", 8),
            ("ETH", 18),
            ("ETH_TEST", 18),
            ("SOL", 9),
            ("SOL_TEST", 9),
            ("TON", 9),
            ("TON_TEST", 9),
            ("USDC_SOL", 6),
            ("USDC_SOL_TEST", 6),
            ("USDC_ERC20", 6),
            ("USDT_ERC20", 6),
            ("USDT_TRC20", 6),
            ("USDT_SOL_TEST", 6),
            ("USDT_ETH_TEST", 6),
            ("ALTXUSDT_TEST", 6),
        ] {
            assert!(decimals.contains(&(code.to_owned(), expected)), "{code}");
        }
        assert_eq!(Currency::fiat(FiatCurrency::USD).decimals(), Some(2));
        assert_eq!(Currency::fiat(FiatCurrency::JPY).decimals(), Some(0));
        assert_eq!(Currency::crypto(CryptoCurrency::Sol).decimals(), Some(9));
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
