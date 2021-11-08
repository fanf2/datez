use anyhow::{anyhow, bail, ensure, Result};
use chrono::{DateTime, NaiveDateTime, TimeZone};
use chrono_tz::Tz;
use std::ffi::OsStr;
use std::path::PathBuf;

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
    bail!("could not parse time {}", arg);
}

fn parse_tz(zone: &str) -> Result<Tz> {
    zone.parse().map_err(|e| anyhow!("could not parse {}: {}", zone, e))
}

fn tz_ok(zone: &OsStr) -> Result<String> {
    let zone = zone.to_str().ok_or_else(|| anyhow!("not utf8"))?;
    parse_tz(zone).map(|_| zone.to_owned())
}

fn get_time(time: &str, zone: &str, tz: &Tz) -> Result<DateTime<Tz>> {
    let naive = parse_time(time)?;
    tz.from_local_datetime(&naive).single().ok_or_else(|| {
        anyhow!("could not convert {} to {} time zone", time, zone)
    })
}

fn print_time_tz(time: &DateTime<Tz>, zone: &str, tz: &Tz) {
    let time = time.with_timezone(tz);
    println!("{} ({})", time.format("%F.%T%z"), zone);
}

fn print_time(time: &DateTime<Tz>, zone: &str) -> Result<()> {
    let tz = parse_tz(zone)?;
    print_time_tz(time, zone, &tz);
    Ok(())
}

fn localzone() -> Result<String> {
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
    bail!("could not find local zone")
}

fn main() -> Result<()> {
    let mut args: Vec<String> = std::env::args().collect();
    if let Ok(zone) = localzone() {
        args.push(zone);
    }
    ensure!(args.len() >= 3, "usage: datez <datetime> <tz>...");
    let time = get_time(&args[1], &args[2], &parse_tz(&args[2])?)?;
    print_time(&time, "UTC")?;
    for arg in args[2..].iter() {
        print_time(&time, arg)?;
    }
    Ok(())
}
