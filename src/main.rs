//! `datez` is a small command-line utility to convert a time into
//! several timezones.
//!
//! ## usage
//!
//!     datez <time> <zone>...
//!
//! You should write the time in ISO 8601 / RFC 3339 format but
//! _without_ a UTC offset, and list as many tz database timezone names
//! as you want.
//!
//! The time is read using the first timezone; it is converted to UTC and
//! printed in UTC and in every timezone you listed, and in your local
//! timezone (if possible).
//!
//! On Unix, the local timezone is discovered from the `TZ` environment
//! variable, or by reading the symlink at `/etc/localtime`; it isn't an
//! error if neither of those work, but you have to list your time zone
//! explicitly.
//!
//! On Windows `datez` gets the local timezone using Win32
//! `GetTimeZoneInformation()`, or falls back to the TZ environment
//! variable.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use std::ffi::OsStr;

#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;

#[cfg(unix)]
use std::path::PathBuf;

/// Try several formats for parsing time
///
/// Example:
/// ```
/// let ndt = parse_time(" 2021-07-21.16:00:00")?;
/// ```
fn parse_time(arg: &str) -> Result<NaiveDateTime> {
    let fmts = [
        "%F.%T",
        "%FT%T",
        "%F %T",
        "%Y%m%d%H%M%S",
        "%Y%m%d.%H%M%S",
        "%Y%m%dT%H%M%S",
        "%Y%m%d %H%M%S",
    ];
    for fmt in fmts {
        if let Ok(time) = NaiveDateTime::parse_from_str(arg, fmt) {
            return Ok(time);
        }
    }
    bail!("time must be in RFC 3339 / ISO 8601 format, without a UTC offset");
}

/// Map an IANA TZDB timezone string into a Tz object
///
fn parse_tz(zone: &str) -> Result<Tz> {
    zone.parse().map_err(|e| anyhow!("{}", e))
}

/// Validate the timezone
///
fn tz_ok(zone: &OsStr) -> Result<String> {
    let zone = zone.to_str().ok_or_else(|| anyhow!("not utf8"))?;
    parse_tz(zone).map(|_| zone.to_owned())
}

/// Parse the time and set its timezone
///
fn get_time(time: &str, zone: &str, tz: &Tz) -> Result<DateTime<Tz>> {
    let naive = parse_time(time)?;
    tz.from_local_datetime(&naive).single().ok_or_else(|| {
        anyhow!("could not convert {} to {} timezone", time, zone)
    })
}

/// Prints the specified time with its plain text timezone
///
fn print_time_tz(time: &DateTime<Tz>, zone: &str, tz: &Tz) {
    let time = time.with_timezone(tz);
    println!("{} ({})", time.format("%F.%T%z"), zone);
}

/// Extracts the timezone before printing the result
///
fn print_time(time: &DateTime<Tz>, zone: &str) -> Result<()> {
    let tz = parse_tz(zone)?;
    print_time_tz(time, zone, &tz);
    Ok(())
}

/// Look for the local timezone on Unix
///
#[cfg(unix)]
fn localzone() -> Result<String> {
    if let Some(zone) = std::env::var_os("TZ") {
        return tz_ok(&zone);
    }
    let path = std::fs::read_link("/etc/localtime")?;
    let mut dir = None;
    let mut leaf = None;
    for name in path.components() {
        dir = leaf;
        leaf = Some(name);
    }
    if let (Some(dir), Some(leaf)) = (dir, leaf) {
        let mut zone = PathBuf::new();
        zone.push(dir.as_os_str());
        zone.push(leaf.as_os_str());
        if let Ok(zone) = tz_ok(zone.as_os_str()) {
            return Ok(zone);
        }
    }
    if let Some(leaf) = leaf {
        return tz_ok(leaf.as_os_str());
    }
    bail!("could not find local timezone")
}

/// Look for the local timezone
///
#[cfg(windows)]
fn localzone() -> Result<String> {
    use std::ffi::OsString;
    use windows::Win32::System::Time::*;

    let mut tz = TIME_ZONE_INFORMATION::default();
    unsafe {
        let e = GetTimeZoneInformation(&mut tz);
        if e == 0 {
            let os_tz = OsString::from_wide(&tz.StandardName[..]);
            let tz = os_tz.as_os_str();

            return tz_ok(tz);
        }
    }
    match std::env::var_os("TZ") {
        Some(tz) => tz_ok(&tz),
        _ => Err(anyhow!("bad TZ")),
    }
}

/// Process the command line
///
fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().collect();
    if let Ok(zone) = localzone() {
        if args.len() == 1 {
            args.push(format!("{}", Local::now().format("%F.%T")));
        }
        args.push(zone);
    }
    if args.len() < 3 || args[1] == "-h" || args[1] == "--help" {
        bail!("usage: datez <datetime> <tz>...");
    }
    let time = get_time(&args[1], &args[2], &parse_tz(&args[2])?)?;
    print_time(&time, "UTC")?;
    for arg in args[2..].iter() {
        print_time(&time, arg)?;
    }
    Ok(())
}

#[cfg(test)]

mod tests {
    use super::*;

    use rstest::rstest;

    #[test]
    fn test_localzone() {
        // this all needs to be one test function, because tests are run
        // in parallel on multiple threads, which is incompatible with
        // manipulating environment variables

        std::env::remove_var("TZ");
        let path = std::fs::read_link("/etc/localtime");
        let zone = localzone();
        match (&path, &zone) {
            (Ok(path), Ok(zone)) => assert!(path.ends_with(zone)),
            (Err(_), Err(_)) => (), // plausible
            _ => panic!("inconsistent localzone: {:?} / {:?}", path, zone),
        }

        std::env::set_var("TZ", "Europe/Paris");
        let tz = localzone();
        assert!(tz.is_ok());
        assert_eq!("Europe/Paris".to_string(), tz.unwrap());
    }

    #[rstest]
    #[case("")]
    #[case("bad")]
    #[case("30001300343433")]
    #[case("30001300 343433")]
    #[case("30001300.343433")]
    #[case("30001300T343433")]
    fn test_parsetime_nok(#[case] s: &str) {
        assert!(parse_time(s).is_err());
    }

    #[rstest]
    #[case("20211201213433")]
    #[case("20211202 213433")]
    #[case("20211203.213433")]
    #[case("20211204T213433")]
    fn test_parsetime_ok(#[case] s: &str) {
        assert!(parse_time(s).is_ok());
    }

    #[rstest]
    #[case("Europe/Paris")]
    #[case("Europe/London")]
    fn test_parse_tz_ok(#[case] s: &str) {
        assert!(parse_tz(s).is_ok());
    }

    #[rstest]
    #[case("Nowhere/None")]
    #[case("Europe/Marseille")]
    fn test_parse_tz_nok(#[case] s: &str) {
        assert!(parse_tz(s).is_err());
    }
}
