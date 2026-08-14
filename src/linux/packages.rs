//! What the package managers know: `dpkg`, `rpm`, `flatpak`, `snap`.
//!
//! A manager that isn't installed simply contributes nothing — each collector
//! returns on the first sign the tool isn't there, so one distro's absence is
//! never another's error.

use super::run;
use crate::*;

pub(super) fn collect_dpkg(out: &mut Vec<Value>) {
    let fmt = "-f=${Package}\t${Version}\t${Maintainer}\t${db:Status-Status}\n";
    let Some(s) = run("dpkg-query", &["-W", fmt]) else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 || f[3] != "installed" || f[0].is_empty() {
            continue;
        }
        out.push(entry("dpkg", f[0].into(), f[1].into(), f[2].into(), String::new(), "system"));
    }
}

pub(super) fn collect_rpm(out: &mut Vec<Value>) {
    let Some(s) = run("rpm", &["-qa", "--qf", "%{NAME}\t%{VERSION}-%{RELEASE}\t%{VENDOR}\n"]) else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        let publisher = f.get(2).copied().filter(|v| *v != "(none)").unwrap_or("");
        out.push(entry(
            "rpm",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            publisher.into(),
            String::new(),
            "system",
        ));
    }
}

pub(super) fn collect_flatpak(out: &mut Vec<Value>) {
    let Some(s) = run("flatpak", &["list", "--app", "--columns=name,version,origin,installation"])
    else {
        return;
    };
    for line in s.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        let scope = if f.get(3) == Some(&"user") { "user" } else { "system" };
        out.push(entry(
            "flatpak",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            f.get(2).copied().unwrap_or("").into(),
            String::new(),
            scope,
        ));
    }
}

pub(super) fn collect_snap(out: &mut Vec<Value>) {
    let Some(s) = run("snap", &["list"]) else {
        return;
    };
    // Columns: Name  Version  Rev  Tracking  Publisher  Notes
    for line in s.lines().skip(1) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() || f[0].is_empty() {
            continue;
        }
        out.push(entry(
            "snap",
            f[0].into(),
            f.get(1).copied().unwrap_or("").into(),
            f.get(4).copied().unwrap_or("").into(),
            String::new(),
            "system",
        ));
    }
}
