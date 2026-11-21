//! Compact month calendar for the Notification Center (Win11-like).

use chrono::{Datelike, Local, NaiveDate};

#[derive(Debug, Clone)]
pub struct MonthGrid {
    pub year: i32,
    pub month: u32,
    /// 0 = Sunday … 6 = Saturday. Empty cells are `None`.
    pub cells: Vec<Option<u32>>,
    pub today_day: Option<u32>,
    pub title: String,
}

impl MonthGrid {
    pub fn current() -> Self {
        let now = Local::now().date_naive();
        Self::for_month(now.year(), now.month())
    }

    pub fn for_month(year: i32, month: u32) -> Self {
        let month = month.clamp(1, 12);
        let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap_or_else(|| Local::now().date_naive());
        let weekday = first.weekday().num_days_from_sunday(); // Sun=0
        let days_in_month = days_in_month(year, month);
        let mut cells: Vec<Option<u32>> = (0..weekday).map(|_| None).collect();
        for day in 1..=days_in_month {
            cells.push(Some(day));
        }
        while cells.len() % 7 != 0 {
            cells.push(None);
        }
        let now = Local::now().date_naive();
        let today_day = if now.year() == year && now.month() == month {
            Some(now.day())
        } else {
            None
        };
        let title = first.format("%B %Y").to_string();
        Self {
            year,
            month,
            cells,
            today_day,
            title,
        }
    }

    pub fn prev(&self) -> Self {
        if self.month <= 1 {
            Self::for_month(self.year - 1, 12)
        } else {
            Self::for_month(self.year, self.month - 1)
        }
    }

    pub fn next(&self) -> Self {
        if self.month >= 12 {
            Self::for_month(self.year + 1, 1)
        } else {
            Self::for_month(self.year, self.month + 1)
        }
    }

    /// ISO week number for a day in this month (W277).
    pub fn week_number(&self, day: u32) -> Option<u32> {
        NaiveDate::from_ymd_opt(self.year, self.month, day).map(|d| d.iso_week().week())
    }

    /// Week number for the current local date when viewing this month; else first day in grid.
    pub fn current_week_label(&self) -> String {
        if let Some(day) = self.today_day {
            if let Some(w) = self.week_number(day) {
                return format!("Week {w}");
            }
        }
        for cell in &self.cells {
            if let Some(day) = *cell {
                if let Some(w) = self.week_number(day) {
                    return format!("Week {w}");
                }
            }
        }
        String::new()
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next = NaiveDate::from_ymd_opt(ny, nm, 1).unwrap();
    let this = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    (next - this).num_days() as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn february_2024_leap() {
        let g = MonthGrid::for_month(2024, 2);
        assert_eq!(g.cells.iter().filter(|c| c.is_some()).count(), 29);
        assert_eq!(g.title, "February 2024");
    }
}
