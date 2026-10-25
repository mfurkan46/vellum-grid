const UNIX_DAYS_TO_SERIAL: i64 = 25569;
const FAKE_LEAP_DAY: i64 = 60;
const SECONDS_PER_DAY: f64 = 86_400.0;
const MAX_SERIAL: f64 = 2_958_466.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (i64::from(month) + 9) % 12;
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

pub fn date_serial(year: i64, month: u32, day: u32) -> Option<f64> {
    if !(1900..=9999).contains(&year) {
        return None;
    }
    if (year, month, day) == (1900, 2, 29) {
        return Some(FAKE_LEAP_DAY as f64);
    }
    let days = days_from_civil(year, month, day);
    if civil_from_days(days) != (year, month, day) {
        return None;
    }
    let serial = days + UNIX_DAYS_TO_SERIAL;
    Some(if serial <= FAKE_LEAP_DAY {
        serial - 1
    } else {
        serial
    } as f64)
}

pub fn time_fraction(hour: u32, minute: u32, second: u32) -> Option<f64> {
    (hour < 24 && minute < 60 && second < 60)
        .then(|| f64::from(hour * 3600 + minute * 60 + second) / SECONDS_PER_DAY)
}

pub fn from_serial(serial: f64) -> Option<DateTime> {
    if !(0.0..MAX_SERIAL).contains(&serial) {
        return None;
    }
    let mut days = serial.floor() as i64;
    let mut seconds = ((serial - serial.floor()) * SECONDS_PER_DAY).round() as u32;
    if seconds == SECONDS_PER_DAY as u32 {
        days += 1;
        seconds = 0;
    }
    let (year, month, day) = match days {
        0 => (1900, 1, 0),
        FAKE_LEAP_DAY => (1900, 2, 29),
        d if d < FAKE_LEAP_DAY => civil_from_days(d + 1 - UNIX_DAYS_TO_SERIAL),
        d => civil_from_days(d - UNIX_DAYS_TO_SERIAL),
    };
    Some(DateTime {
        year,
        month,
        day,
        hour: seconds / 3600,
        minute: seconds / 60 % 60,
        second: seconds % 60,
    })
}

pub fn today_serial(unix_millis: f64) -> f64 {
    (unix_millis / (SECONDS_PER_DAY * 1000.0)).floor() + UNIX_DAYS_TO_SERIAL as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_serials_match_1900_system() {
        assert_eq!(date_serial(1900, 1, 1), Some(1.0));
        assert_eq!(date_serial(1900, 2, 28), Some(59.0));
        assert_eq!(date_serial(1900, 2, 29), Some(60.0));
        assert_eq!(date_serial(1900, 3, 1), Some(61.0));
        assert_eq!(date_serial(1970, 1, 1), Some(25569.0));
        assert_eq!(date_serial(2026, 10, 17), Some(46312.0));
        assert_eq!(date_serial(9999, 12, 31), Some(2_958_465.0));
    }

    #[test]
    fn rejects_impossible_dates() {
        assert_eq!(date_serial(2025, 2, 29), None);
        assert_eq!(date_serial(2026, 13, 1), None);
        assert_eq!(date_serial(2026, 4, 31), None);
        assert_eq!(date_serial(1899, 12, 31), None);
        assert_eq!(date_serial(10000, 1, 1), None);
        assert_eq!(date_serial(2024, 2, 29), Some(45351.0));
    }

    #[test]
    fn serials_round_trip() {
        for serial in [
            1.0,
            59.0,
            60.0,
            61.0,
            366.0,
            25569.0,
            45351.0,
            46312.0,
            2_958_465.0,
        ] {
            let dt = from_serial(serial).unwrap();
            assert_eq!(
                date_serial(dt.year, dt.month, dt.day),
                Some(serial),
                "{dt:?}"
            );
        }
    }

    #[test]
    fn splits_time_of_day() {
        let dt = from_serial(46312.0 + time_fraction(13, 45, 30).unwrap()).unwrap();
        assert_eq!((dt.year, dt.month, dt.day), (2026, 10, 17));
        assert_eq!((dt.hour, dt.minute, dt.second), (13, 45, 30));
        let midnight = from_serial(46_312.999_999_9).unwrap();
        assert_eq!((midnight.day, midnight.hour), (18, 0));
        assert_eq!(time_fraction(24, 0, 0), None);
        assert_eq!(from_serial(-1.0), None);
    }

    #[test]
    fn today_from_unix_time() {
        assert_eq!(today_serial(0.0), 25569.0);
        assert_eq!(today_serial(1_792_246_314_506.0), 46312.0);
    }
}
