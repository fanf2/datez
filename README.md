datez: convert and display a time across several timezones
==========================================================

In July, when it is 16:00 in Canberra, what time is it in Caracas, and
where I am in Cambridge?

        $ datez 2021-07-21.16:00:00 Australia/Canberra America/Caracas
        2021-07-21.06:00:00+0000 (UTC)
        2021-07-21.16:00:00+1000 (Australia/Canberra)
        2021-07-21.02:00:00-0400 (America/Caracas)
        2021-07-21.07:00:00+0100 (Europe/London)


install
-------

`datez` is written in Rust, so use [`cargo install`][cargo].

[cargo]: https://doc.rust-lang.org/cargo/commands/cargo-install.html


usage
-----

        datez <time> <zone>...

Write the time in ISO 8601 / RFC 3339 format but without a UTC offset,
and list as many tz database timezone names as you want.

The time is read using the first timezone; it is converted to UTC and
printed in UTC and in every timezone you listed, and in your local
time zone (if possible).

The local time zone is discovered from the `TZ` environment variable
or by reading the symlink at `/etc/localtime`; if neither of those
work you have to list your time zone explicitly.


licence
-------

> This was written by Tony Finch <<dot@dotat.at>>  
> You may do anything with it. It has no warranty.  
> <https://creativecommons.org/publicdomain/zero/1.0/>  
> SPDX-License-Identifier: CC0-1.0
