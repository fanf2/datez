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
//! The local timezone is discovered from the `TZ` environment variable
//! if that is set, or by an OS-specific mechanism; it isn't an error
//! if neither of those work, but you have to list your timezone
//! explicitly.
//!
//! On Unix, `datez` reads the symlink at `/etc/localtime`.
//!
//! On Windows, `datez` calls Win32 `GetTimeZoneInformation()`.

use anyhow::{anyhow, bail, Result};
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use std::collections::HashMap;
use std::ffi::OsStr;
#[cfg(windows)]
use std::ffi::OsString;

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

/// Look for the local timezone
///
fn localzone() -> Result<String> {
    if let Some(zone) = std::env::var_os("TZ") {
        tz_ok(&zone)
    } else {
        localzone_os()
    }
}

/// Look for the local timezone using `/etc/localtime`
///
#[cfg(unix)]
fn localzone_os() -> Result<String> {
    use std::path::PathBuf;

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
    // try single-part timezone names such as "UTC"
    if let Some(leaf) = leaf {
        return tz_ok(leaf.as_os_str());
    }
    bail!("could not find local timezone")
}

/// Remove trailing \u{0} from \u16 string returned by Windows
/// Inspired from https://github.com/retep998/wio-rs/blob/master/src/wide.rs
#[cfg(windows)]
fn from_wide_null(wide: &[u16]) -> OsString {
    use std::os::windows::ffi::OsStringExt;

    let len = wide.iter().take_while(|&&c| c != 0).count();
    OsString::from_wide(&wide[..len])
}

/// Look for the local timezone using `GetTimeZoneInformation()`
///
#[cfg(windows)]
fn localzone_os() -> Result<String> {
    use windows::Win32::System::Time::*;

    let mut tz = TIME_ZONE_INFORMATION::default();
    let e = unsafe { GetTimeZoneInformation(&mut tz) };
    match e {
        0 | 1 | 2 => {
            let zone = from_wide_null(&tz.StandardName[..]);
            let zone = zone.to_str();
            // Fix some timezones
            match zone {
                Some(s) => canonize_tz(s),
                _ => bail!("could not find local timezone"),
            }
        }
        _ => bail!("could not find local timezone"),
    }
}

/// Windows timezones are in some case completely different from the rest of world
/// so fix it for known cases.
#[cfg(windows)]
fn canonize_tz(zone: &str) -> Result<String> {
    // XXX will probably evolve into a hash if other cases appear
    if zone == "Romance Standard Time" {
        return Ok("Europe/Paris".to_string());
    }
    let z = OsStr::new(zone);
    tz_ok(z)
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
    let mut dedup = HashMap::new();
    print_time(&time, "UTC")?;
    dedup.insert("UTC".to_string(), ());
    for arg in args[2..].iter() {
        if !dedup.contains_key(arg) {
            print_time(&time, arg)?;
            dedup.insert(arg.to_string(), ());
        }
    }
    Ok(())
}

#[cfg(test)]

mod tests {
    use super::*;

    use rstest::rstest;

    #[test]
    #[cfg(unix)]
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
