//! Write the time in ISO 8601 / RFC 3339 format but without a UTC offset,
//! and list as many tz database timezone names as you want.
//!
//! The time is read using the first timezone; it is converted to UTC and
//! printed in UTC and in every timezone you listed, and in your local
//! time zone (if possible).
//!
//! The local time zone is discovered from the `TZ` environment variable
//! or by reading the symlink at `/etc/localtime`; if neither of those
//! work you have to list your time zone explicitly.
//!
//! On Windows we use one Win32 syscalls or fallback to TZ as well.
//!

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

fn parse_tz(zone: &str) -> Result<Tz> {
    zone.parse().map_err(|e| anyhow!("{}", e))
}

/// Validate the timezone
///
fn tz_ok(zone: &OsStr) -> Result<String> {
    let zone = zone.to_str().ok_or_else(|| anyhow!("not utf8"))?;
    parse_tz(zone).map(|_| zone.to_owned())
}

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

/// Extracts the time zone before printing the result
///
fn print_time(time: &DateTime<Tz>, zone: &str) -> Result<()> {
    let tz = parse_tz(zone)?;
    print_time_tz(time, zone, &tz);
    Ok(())
}

/// Look for the local timezone on Unix
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

/// Windows version of `localzone`.
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
    fn test_localzone_tz() {
        std::env::remove_var("TZ");

        std::env::set_var("TZ", "Europe/Paris");
        let tz = localzone();
        assert!(tz.is_ok());

        assert_eq!("Europe/Paris".to_string(), tz.unwrap());
    }

    #[test]
    #[cfg(unix)]
    fn test_localzone_notz() {
        std::env::remove_var("TZ");

        let tz = localzone();
        println!("{:?}", tz);
        assert!(tz.is_ok());

        assert_eq!("Europe/Paris".to_string(), tz.unwrap());
    }

    #[rstest]
    #[case("")]
    #[case("bad")]
    #[case("3000:13:0034:34:33")]
    #[case("3000:13:00 34:34:33")]
    #[case("3000:13:00.34:34:33")]
    #[case("3000:13:00T34:34:33")]
    fn test_parsetime_bad(#[case] s: &str) {
        assert!(parse_time(s).is_err());
    }
}
