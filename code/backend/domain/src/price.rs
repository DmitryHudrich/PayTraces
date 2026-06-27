use crate::primitives::Amount;

/// USD value with cent precision.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct UsdAmount(f64);

impl UsdAmount {
    pub fn new(value: f64) -> Self {
        Self(value.max(0.0))
    }

    pub fn zero() -> Self {
        Self(0.0)
    }

    pub fn value(self) -> f64 {
        self.0
    }
}

impl std::ops::Add for UsdAmount {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}

/// USD unit price for one whole token (decimals already taken into account).
#[derive(Debug, Clone, Copy)]
pub struct UnitPrice(pub f64);

impl UnitPrice {
    pub fn apply(self, amount: Amount) -> UsdAmount {
        let whole = amount.raw().to_string().parse::<f64>().unwrap_or(0.0)
            / 10f64.powi(amount.decimals() as i32);
        UsdAmount::new(whole * self.0)
    }
}
