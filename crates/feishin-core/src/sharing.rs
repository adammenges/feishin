use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Days, Months, TimeZone};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    #[default]
    Year,
}
impl Unit {
    pub const ALL: [Self; 7] = [
        Self::Second,
        Self::Minute,
        Self::Hour,
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Year,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Second => "Seconds",
            Self::Minute => "Minutes",
            Self::Hour => "Hours",
            Self::Day => "Days",
            Self::Week => "Weeks",
            Self::Month => "Months",
            Self::Year => "Years",
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct Expiration {
    pub amount: u32,
    pub unit: Unit,
    pub use_server_default: bool,
}
impl Default for Expiration {
    fn default() -> Self {
        Self {
            amount: 1,
            unit: Unit::Year,
            use_server_default: false,
        }
    }
}
impl Expiration {
    pub fn deadline<Tz: TimeZone>(&self, now: DateTime<Tz>) -> Result<Option<DateTime<Tz>>> {
        ensure!(self.amount > 0, "Share expiration amount must be positive");
        if self.use_server_default {
            return Ok(None);
        }
        let amount = self.amount as i64;
        let date = match self.unit {
            Unit::Second => now.checked_add_signed(chrono::Duration::seconds(amount)),
            Unit::Minute => now.checked_add_signed(chrono::Duration::minutes(amount)),
            Unit::Hour => now.checked_add_signed(chrono::Duration::hours(amount)),
            Unit::Day => now.checked_add_days(Days::new(self.amount as u64)),
            Unit::Week => now.checked_add_days(Days::new(self.amount as u64 * 7)),
            Unit::Month => now.checked_add_months(Months::new(self.amount)),
            Unit::Year => self
                .amount
                .checked_mul(12)
                .and_then(|months| now.checked_add_months(Months::new(months))),
        }
        .context("Share expiration is outside the supported date range")?;
        Ok(Some(date))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    #[test]
    fn calendar_expiration_handles_leap_months_units_defaults_and_overflow() {
        let jan = Utc.with_ymd_and_hms(2024, 1, 31, 12, 30, 0).unwrap();
        for (unit, expected) in [
            (Unit::Second, "2024-01-31 12:30:01"),
            (Unit::Minute, "2024-01-31 12:31:00"),
            (Unit::Hour, "2024-01-31 13:30:00"),
            (Unit::Day, "2024-02-01 12:30:00"),
            (Unit::Week, "2024-02-07 12:30:00"),
            (Unit::Month, "2024-02-29 12:30:00"),
            (Unit::Year, "2025-01-31 12:30:00"),
        ] {
            let expiration = Expiration {
                unit,
                ..Default::default()
            };
            assert_eq!(
                expiration
                    .deadline(jan)
                    .unwrap()
                    .unwrap()
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
                expected
            );
        }
        let leap = Utc.with_ymd_and_hms(2024, 2, 29, 12, 30, 0).unwrap();
        assert_eq!(
            Expiration::default().deadline(leap).unwrap().unwrap().day(),
            28
        );
        assert!(
            Expiration {
                use_server_default: true,
                ..Default::default()
            }
            .deadline(jan)
            .unwrap()
            .is_none()
        );
        assert!(
            Expiration {
                amount: 0,
                ..Default::default()
            }
            .deadline(jan)
            .is_err()
        );
        assert!(
            Expiration {
                amount: u32::MAX,
                ..Default::default()
            }
            .deadline(jan)
            .is_err()
        );
        use chrono::Datelike;
    }
}
