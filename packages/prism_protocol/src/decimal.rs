//
// base Decimal by alwin-peng
//

use core::ops;
use cosmwasm_std::{
    Decimal as StdDecimal, Decimal256, Fraction, StdError, StdResult, Uint256, Uint512,
};
use schemars::JsonSchema;
use serde::{de, ser, Deserialize, Deserializer, Serialize};
use std::convert::{TryFrom, TryInto};
use std::fmt::{self};
use std::ops::{Add, Div, Mul, Sub};
use std::str::FromStr;

#[derive(Copy, Clone, Debug, Eq, PartialOrd, Ord, JsonSchema)]
pub struct Decimal {
    pub positive: bool,
    pub decimal: Decimal256,
}

impl Decimal {
    pub const DECIMAL_FRACTIONAL: Uint256 = // 1*10**18
        Uint256::from_be_bytes([
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 13, 224, 182,
            179, 167, 100, 0, 0,
        ]);

    pub const MAX: Self = Decimal {
        decimal: Decimal256::MAX,
        positive: true,
    };

    pub const fn one() -> Self {
        Self {
            decimal: Decimal256::one(),
            positive: true,
        }
    }

    pub const fn zero() -> Self {
        Self {
            decimal: Decimal256::zero(),
            positive: true,
        }
    }

    pub fn floor(&self) -> Self {
        Decimal {
            decimal: Decimal256::from_ratio(self.u256(), Uint256::from(1u8)),
            positive: self.positive,
        }
    }

    pub fn ceil(&self) -> Self {
        if *self == self.floor() {
            return *self;
        }

        Decimal {
            decimal: Decimal256::from_ratio(self.u256() + Uint256::from(1u8), Uint256::from(1u8)),
            positive: self.positive,
        }
    }

    pub fn assert_int(&self) -> StdResult<()> {
        if self.floor() != *self {
            Err(StdError::generic_err("invalid type: integer requested"))
        } else {
            Ok(())
        }
    }

    pub fn assert_positive(&self) -> StdResult<()> {
        if self.positive {
            Ok(())
        } else {
            Err(StdError::generic_err(
                "invalid type: positive integer requested",
            ))
        }
    }

    pub fn assert_uint(&self) -> StdResult<()> {
        self.assert_positive()?;
        self.assert_int()
    }

    pub fn abs(&self) -> Decimal {
        Self {
            decimal: self.decimal,
            positive: true,
        }
    }

    pub fn u256(&self) -> Uint256 {
        self.decimal.numerator() / Decimal::DECIMAL_FRACTIONAL
    }

    pub fn u128(&self) -> u128 {
        if !self.positive {
            panic!("attempting to convert negative decimal to u128")
        }
        let val = self.decimal.numerator() / Decimal::DECIMAL_FRACTIONAL;
        let as_be = val.to_be_bytes();
        u128::from_be_bytes(as_be[16..32].try_into().unwrap())
    }

    pub fn is_zero(&self) -> bool {
        self.decimal.is_zero()
    }

    /// Convert x% into Decimal
    pub fn percent(x: u64) -> Self {
        Decimal {
            decimal: Decimal256::percent(x),
            positive: true,
        }
    }

    /// Convert permille (x/1000) into Decimal
    pub fn permille(x: u64) -> Self {
        Decimal {
            decimal: Decimal256::permille(x),
            positive: true,
        }
    }

    pub fn inv(&self) -> Option<Self> {
        if self.is_zero() {
            None
        } else {
            Some(Decimal {
                decimal: self.decimal.inv().unwrap(),
                positive: self.positive,
            })
        }
    }

    pub fn pow(&self, exp: i64) -> Self {
        let mut res = Decimal::one();
        let mut current = *self;

        let mut bin = exp.abs() as u64;
        while bin != 0u64 {
            if bin & 1 == 1 {
                res *= current;
            }
            current *= current;
            bin >>= 1;
        }

        if exp.is_negative() {
            res = res.inv().unwrap();
        }
        res
    }

    pub fn log(&self, base: Decimal) -> i64 {
        if !self.positive || !base.positive {
            panic!("taking logarithm of negative number or negative base is not supported")
        }

        if *self == Decimal::one() {
            return 0;
        }

        if *self < Decimal::one() {
            return -self.inv().unwrap().log(base);
        }

        let mut values = vec![base];
        while values.last().unwrap() < self {
            values.push(*values.last().unwrap() * *values.last().unwrap())
        }

        let mut current = Decimal::one();
        let mut out = 0i64;

        for i in (0..values.len()).rev() {
            // values[i] -> base**(1 << i)
            if current * values[i] <= *self {
                current *= values[i];
                out += 1 << i;
            }
        }

        out
    }

    pub fn sqrt(&self) -> Self {
        if !self.positive {
            panic!("may not take square root of negative decimal",)
        } else {
            Self {
                decimal: self.decimal.sqrt(),
                positive: true,
            }
        }
    }

    /// Returns the ratio (numerator / denominator) as a Decimal
    pub fn from_ratio(numerator: impl Into<Uint256>, denominator: impl Into<Uint256>) -> Self {
        Self {
            decimal: Decimal256::from_ratio(numerator, denominator),
            positive: true,
        }
    }

    pub fn from_decimal(decimal: StdDecimal) -> Self {
        Self::from_str(decimal.to_string().as_str()).unwrap()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = vec![];
        v.extend_from_slice(&self.decimal.numerator().to_be_bytes());
        v.extend_from_slice(&(self.positive as u8).to_be_bytes());
        v
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let numerator = Uint256::from_be_bytes(bytes[0..32].try_into().unwrap());
        let positive = *bytes.last().unwrap() == 1;
        Self {
            positive,
            decimal: Decimal256::from_ratio(numerator, 1u8),
        }
    }
}

impl Default for Decimal {
    fn default() -> Self {
        Decimal::zero()
    }
}

impl FromStr for Decimal {
    type Err = StdError;

    /// Converts the decimal string to a Decimal
    /// Possible inputs: "1.23", "1", "000012", "1.123000000"
    /// Disallowed: "", ".23"
    ///
    /// This never performs any kind of rounding.
    /// More than DECIMAL_PLACES fractional digits, even zeros, result in an error.
    fn from_str(input: &str) -> StdResult<Self> {
        let mut to_parse = &(*input);
        let mut positive = true;
        if input.starts_with('-') {
            positive = false;
            to_parse = &to_parse[1..];
        }

        Ok(Self {
            decimal: Decimal256::from_str(to_parse)?,
            positive,
        })
    }
}

impl fmt::Display for Decimal {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if !self.positive {
            write!(f, "-")?
        }
        self.decimal.fmt(f)
    }
}

impl PartialEq for Decimal {
    fn eq(&self, other: &Self) -> bool {
        if self.is_zero() && other.is_zero() {
            return true;
        }
        self.decimal == other.decimal && self.positive == other.positive
    }
}

impl ops::Neg for Decimal {
    type Output = Self;

    fn neg(self) -> Self {
        Decimal {
            decimal: self.decimal,
            positive: !self.positive,
        }
    }
}

impl ops::Add for Decimal {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if self.positive == other.positive {
            Decimal {
                decimal: self.decimal + other.decimal,
                positive: self.positive,
            }
        } else {
            let bigger = if self.decimal > other.decimal {
                self
            } else {
                other
            };
            let smaller = if self.decimal < other.decimal {
                self
            } else {
                other
            };
            Decimal {
                decimal: bigger.decimal - smaller.decimal,
                positive: bigger.positive,
            }
        }
    }
}

impl ops::AddAssign for Decimal {
    fn add_assign(&mut self, other: Self) {
        *self = self.add(other)
    }
}

impl ops::Sub for Decimal {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        self + (-other)
    }
}

impl ops::SubAssign for Decimal {
    fn sub_assign(&mut self, other: Self) {
        *self = self.sub(other)
    }
}

impl ops::Mul for Decimal {
    type Output = Decimal;

    fn mul(self, rhs: Decimal) -> Self::Output {
        let result = Uint512::from(self.decimal.numerator())
            * Uint512::from(rhs.decimal.numerator())
            / Uint512::from(Decimal::DECIMAL_FRACTIONAL);
        let dec_internal = Uint256::try_from(result).unwrap();

        Decimal {
            decimal: Decimal256::from_ratio(dec_internal, Decimal::DECIMAL_FRACTIONAL),
            positive: self.positive == rhs.positive,
        }
    }
}

impl ops::MulAssign for Decimal {
    fn mul_assign(&mut self, other: Self) {
        *self = self.mul(other)
    }
}

impl ops::Div for Decimal {
    type Output = Self;

    fn div(self, rhs: Decimal) -> Self::Output {
        let result = Uint512::from(self.decimal.numerator())
            * Uint512::from(Decimal::DECIMAL_FRACTIONAL)
            / Uint512::from(rhs.decimal.numerator());
        let dec_internal = Uint256::try_from(result).unwrap();
        Decimal {
            decimal: Decimal256::from_ratio(dec_internal, Decimal::DECIMAL_FRACTIONAL),
            positive: self.positive == rhs.positive,
        }
    }
}

impl ops::DivAssign for Decimal {
    fn div_assign(&mut self, other: Self) {
        *self = self.div(other)
    }
}

/// Serializes as a decimal string
impl Serialize for Decimal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Deserializes as a base64 string
impl<'de> Deserialize<'de> for Decimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(DecimalVisitor)
    }
}

struct DecimalVisitor;

impl<'de> de::Visitor<'de> for DecimalVisitor {
    type Value = Decimal;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("string-encoded decimal")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        match Decimal::from_str(v) {
            Ok(d) => Ok(d),
            Err(e) => Err(E::custom(format!("Error parsing decimal '{}': {}", v, e))),
        }
    }
}
