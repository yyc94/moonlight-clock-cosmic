use chrono::{DateTime, Datelike, Duration, FixedOffset, NaiveDate, TimeZone, Utc};

const THRESHOLDS: [(&str, f64); 4] = [("new", 0.0), ("first", 0.25), ("full", 0.5), ("last", 0.75)];

pub fn moon_illumination_phase(moment: DateTime<Utc>) -> f64 {
    let rad = std::f64::consts::PI / 180.0;
    let days = moment.timestamp_millis() as f64 / 86_400_000.0 - 0.5 + 2_440_588.0 - 2_451_545.0;
    let solar_anomaly = rad * (357.5291 + 0.98560028 * days);
    let center = rad
        * (1.9148 * solar_anomaly.sin()
            + 0.02 * (2.0 * solar_anomaly).sin()
            + 0.0003 * (3.0 * solar_anomaly).sin());
    let solar_longitude = solar_anomaly + center + rad * 102.9372 + std::f64::consts::PI;
    let (solar_ra, solar_dec) = equatorial(solar_longitude, 0.0);

    let lunar_longitude = rad * (218.316 + 13.176396 * days);
    let lunar_anomaly = rad * (134.963 + 13.064993 * days);
    let lunar_distance_arg = rad * (93.272 + 13.229350 * days);
    let longitude = lunar_longitude + rad * 6.289 * lunar_anomaly.sin();
    let latitude = rad * 5.128 * lunar_distance_arg.sin();
    let distance = 385_001.0 - 20_905.0 * lunar_anomaly.cos();
    let (lunar_ra, lunar_dec) = equatorial(longitude, latitude);

    let separation = (solar_dec.sin() * lunar_dec.sin()
        + solar_dec.cos() * lunar_dec.cos() * (solar_ra - lunar_ra).cos())
    .acos();
    let incidence =
        (149_598_000.0 * separation.sin()).atan2(distance - 149_598_000.0 * separation.cos());
    let angle = (solar_dec.cos() * (solar_ra - lunar_ra).sin()).atan2(
        solar_dec.sin() * lunar_dec.cos()
            - solar_dec.cos() * lunar_dec.sin() * (solar_ra - lunar_ra).cos(),
    );
    0.5 + 0.5 * incidence * if angle < 0.0 { -1.0 } else { 1.0 } / std::f64::consts::PI
}

pub fn phase_code(day: NaiveDate) -> &'static str {
    let phase = phase_for_day(day);
    let previous = phase_for_day(day - Duration::days(1));
    let following = phase_for_day(day + Duration::days(1));
    for (name, threshold) in THRESHOLDS {
        if closest_crossing(phase, previous, threshold)
            || closest_crossing(phase, following, threshold)
        {
            return name;
        }
    }
    if phase < 0.25 {
        "new-first"
    } else if phase < 0.5 {
        "first-full"
    } else if phase < 0.75 {
        "full-last"
    } else {
        "last-new"
    }
}

pub fn next_quarter(day: NaiveDate, targets: &[String]) -> (NaiveDate, &'static str) {
    for offset in 0..=60 {
        let candidate = day + Duration::days(offset);
        let code = phase_code(candidate);
        if targets.iter().any(|target| target == code) {
            return (candidate, code);
        }
    }
    (day + Duration::days(60), "full")
}

fn phase_for_day(day: NaiveDate) -> f64 {
    let offset = FixedOffset::east_opt(8 * 3600).unwrap();
    let moment = offset
        .with_ymd_and_hms(day.year(), day.month(), day.day(), 12, 0, 0)
        .single()
        .unwrap()
        .with_timezone(&Utc);
    moon_illumination_phase(moment)
}

fn closest_crossing(mut current: f64, mut other: f64, threshold: f64) -> bool {
    if threshold == 0.0 {
        if current > 0.5 {
            current -= 1.0;
        }
        if other > 0.5 {
            other -= 1.0;
        }
    }
    let current_diff = threshold - current;
    let other_diff = threshold - other;
    current_diff * other_diff <= 0.0
        && current_diff.abs() <= 0.125
        && current_diff.abs() <= other_diff.abs()
}

fn equatorial(longitude: f64, latitude: f64) -> (f64, f64) {
    let obliquity = std::f64::consts::PI / 180.0 * 23.4397;
    let right_ascension = (longitude.sin() * obliquity.cos() - latitude.tan() * obliquity.sin())
        .atan2(longitude.cos());
    let declination = (latitude.sin() * obliquity.cos()
        + latitude.cos() * obliquity.sin() * longitude.sin())
    .asin();
    (right_ascension, declination)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_quarters() {
        assert_eq!(
            phase_code(NaiveDate::from_ymd_opt(2024, 3, 25).unwrap()),
            "full"
        );
        assert_eq!(
            phase_code(NaiveDate::from_ymd_opt(2024, 4, 9).unwrap()),
            "new"
        );
        let target = vec!["full".into()];
        assert_eq!(
            next_quarter(NaiveDate::from_ymd_opt(2024, 4, 10).unwrap(), &target),
            (NaiveDate::from_ymd_opt(2024, 4, 24).unwrap(), "full")
        );
    }
}
