// @author Cursor

use std::cmp::Ordering;
use std::collections::HashMap;

use serde_json::Value;

use crate::catalog::LineKey;
use crate::model::{LineSupport, NewerLine, ReleaseLine, VersionStatus};
use crate::version::{judge, to_semver, version_token};

pub struct LineJudgement {
    pub line: String,
    pub latest_version: String,
    pub status: VersionStatus,
    pub newer: Vec<NewerLine>,
}

pub fn judge_lines(current: &str, lines: &[ReleaseLine], key: LineKey) -> LineJudgement {
    let Some(user_id) = version_line_id(current, key) else {
        return unknown();
    };
    if lines.is_empty() {
        return unknown();
    }

    let Some(line) = find_line(lines, &user_id, key) else {
        if lines
            .iter()
            .all(|line| cmp_line_id(&user_id, &line.id) == Ordering::Greater)
        {
            return LineJudgement {
                line: user_id,
                latest_version: String::new(),
                status: VersionStatus::Unknown,
                newer: Vec::new(),
            };
        }
        return LineJudgement {
            line: user_id.clone(),
            latest_version: String::new(),
            status: VersionStatus::Unsupported,
            newer: newer_than(&user_id, lines),
        };
    };

    let status = if line.support == LineSupport::Eol {
        VersionStatus::Unsupported
    } else {
        judge(current, &line.latest)
    };
    LineJudgement {
        line: line.label.clone(),
        latest_version: line.latest.clone(),
        status,
        newer: newer_than(&line.id, lines),
    }
}

pub fn single_line(latest: &str) -> Vec<ReleaseLine> {
    vec![ReleaseLine {
        id: "stable".into(),
        label: "stable".into(),
        latest: latest.into(),
        support: LineSupport::Stable,
    }]
}

pub fn lines_from_node(
    index: &str,
    endoflife: Option<&str>,
    today: &str,
) -> Option<Vec<ReleaseLine>> {
    let indexed = node_index(index)?;
    if indexed.is_empty() {
        return None;
    }
    let cycles = endoflife
        .and_then(|body| raw_cycles(body, today))
        .unwrap_or_default();
    if cycles.is_empty() {
        return Some(finish(lines_from_index_only(&indexed)));
    }

    let mut lines = assign_supports(&cycles, |cycle| {
        indexed
            .get(&cycle.id)
            .and_then(|info| info.codename.clone())
    });
    for (id, info) in &indexed {
        if lines.iter().any(|line| &line.id == id) {
            if let Some(line) = lines.iter_mut().find(|line| &line.id == id) {
                if version_is_newer(&info.latest, &line.latest) {
                    line.latest = info.latest.clone();
                }
            }
            continue;
        }
        lines.push(index_only_line(id, info, &indexed));
    }
    Some(finish(lines))
}

pub fn lines_from_endoflife(body: &str, today: &str) -> Option<Vec<ReleaseLine>> {
    let cycles = raw_cycles(body, today)?;
    if cycles.is_empty() {
        return None;
    }
    Some(finish(assign_supports(&cycles, |_| None)))
}

pub fn lines_from_go(body: &str) -> Option<Vec<ReleaseLine>> {
    let releases: Vec<Value> = serde_json::from_str(body).ok()?;
    let mut latest_by_minor: HashMap<String, String> = HashMap::new();
    for release in releases {
        if release.get("stable").and_then(Value::as_bool) == Some(false) {
            continue;
        }
        let Some(version) = release.get("version").and_then(Value::as_str) else {
            continue;
        };
        let Some(token) = version_token(version) else {
            continue;
        };
        let Some(id) = version_line_id(&token, LineKey::MajorMinor) else {
            continue;
        };
        latest_by_minor
            .entry(id)
            .and_modify(|current| {
                if version_is_newer(&token, current) {
                    *current = token.clone();
                }
            })
            .or_insert(token);
    }
    if latest_by_minor.is_empty() {
        return None;
    }
    let newest = latest_by_minor
        .keys()
        .max_by(|left, right| cmp_line_id(left, right))
        .cloned();
    let lines = latest_by_minor
        .into_iter()
        .map(|(id, latest)| {
            let current = newest.as_deref() == Some(id.as_str());
            ReleaseLine {
                label: if current {
                    format!("{id} Current")
                } else {
                    id.clone()
                },
                support: if current {
                    LineSupport::Current
                } else {
                    LineSupport::Maintenance
                },
                id,
                latest,
            }
        })
        .collect();
    Some(finish(lines))
}

pub fn today_iso(unix_secs: u64) -> String {
    let (year, month, day) = civil_from_unix_days((unix_secs / 86_400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

struct IndexedMajor {
    latest: String,
    codename: Option<String>,
}

struct RawCycle {
    id: String,
    latest: String,
    eol: bool,
    lts: bool,
    support_active: bool,
}

fn node_index(body: &str) -> Option<HashMap<String, IndexedMajor>> {
    let releases: Vec<Value> = serde_json::from_str(body).ok()?;
    let mut indexed = HashMap::<String, IndexedMajor>::new();
    for release in releases {
        let Some(version) = release.get("version").and_then(Value::as_str) else {
            continue;
        };
        if version.contains('-') {
            continue;
        }
        let Some(token) = version_token(version) else {
            continue;
        };
        let Some(id) = version_line_id(&token, LineKey::Major) else {
            continue;
        };
        let codename = release
            .get("lts")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .map(str::to_string);
        match indexed.get_mut(&id) {
            Some(current) => {
                if version_is_newer(&token, &current.latest) {
                    current.latest = token;
                    if codename.is_some() {
                        current.codename = codename;
                    }
                } else if current.codename.is_none() {
                    current.codename = codename;
                }
            }
            None => {
                indexed.insert(
                    id,
                    IndexedMajor {
                        latest: token,
                        codename,
                    },
                );
            }
        }
    }
    Some(indexed)
}

fn lines_from_index_only(indexed: &HashMap<String, IndexedMajor>) -> Vec<ReleaseLine> {
    indexed
        .iter()
        .map(|(id, info)| index_only_line(id, info, indexed))
        .collect()
}

fn index_only_line(
    id: &str,
    info: &IndexedMajor,
    indexed: &HashMap<String, IndexedMajor>,
) -> ReleaseLine {
    let newest = indexed
        .keys()
        .max_by(|left, right| cmp_line_id(left, right));
    let newest_lts = indexed
        .iter()
        .filter(|(_, info)| info.codename.is_some())
        .map(|(id, _)| id)
        .max_by(|left, right| cmp_line_id(left, right));
    let support = if info.codename.is_some() {
        if newest_lts.map(String::as_str) == Some(id) {
            LineSupport::ActiveLts
        } else {
            LineSupport::Maintenance
        }
    } else if newest.map(String::as_str) == Some(id) {
        LineSupport::Current
    } else {
        LineSupport::Maintenance
    };
    ReleaseLine {
        label: label_for(id, support, info.lts_started(), info.codename.as_deref()),
        id: id.to_string(),
        latest: info.latest.clone(),
        support,
    }
}

impl IndexedMajor {
    fn lts_started(&self) -> bool {
        self.codename.is_some()
    }
}

fn raw_cycles(body: &str, today: &str) -> Option<Vec<RawCycle>> {
    let cycles: Vec<Value> = serde_json::from_str(body).ok()?;
    let mut parsed = Vec::new();
    for cycle in cycles {
        let Some(id) = cycle.get("cycle").and_then(Value::as_str) else {
            continue;
        };
        let Some(latest) = cycle
            .get("latest")
            .and_then(Value::as_str)
            .and_then(version_token)
        else {
            continue;
        };
        let eol = is_eol(cycle.get("eol"), today);
        let lts = lts_started(cycle.get("lts"), today);
        let support_active = support_is_active(cycle.get("support"), today);
        parsed.push(RawCycle {
            id: id.to_string(),
            latest,
            eol,
            lts,
            support_active,
        });
    }
    Some(parsed)
}

fn assign_supports(
    cycles: &[RawCycle],
    codename: impl Fn(&RawCycle) -> Option<String>,
) -> Vec<ReleaseLine> {
    let newest = cycles
        .iter()
        .filter(|cycle| !cycle.eol)
        .map(|cycle| cycle.id.as_str())
        .max_by(|left, right| cmp_line_id(left, right))
        .map(str::to_string);
    let newest_lts = cycles
        .iter()
        .filter(|cycle| !cycle.eol && cycle.lts)
        .map(|cycle| cycle.id.as_str())
        .max_by(|left, right| cmp_line_id(left, right))
        .map(str::to_string);
    cycles
        .iter()
        .map(|cycle| {
            let support = if cycle.eol {
                LineSupport::Eol
            } else if cycle.lts {
                if newest_lts.as_deref() == Some(cycle.id.as_str()) && cycle.support_active {
                    LineSupport::ActiveLts
                } else {
                    LineSupport::Maintenance
                }
            } else if newest.as_deref() == Some(cycle.id.as_str()) {
                LineSupport::Current
            } else {
                LineSupport::Maintenance
            };
            let code = codename(cycle);
            ReleaseLine {
                label: label_for(&cycle.id, support, cycle.lts, code.as_deref()),
                id: cycle.id.clone(),
                latest: cycle.latest.clone(),
                support,
            }
        })
        .collect()
}

fn label_for(id: &str, support: LineSupport, lts: bool, codename: Option<&str>) -> String {
    match support {
        LineSupport::Current => format!("{id} Current"),
        LineSupport::Stable => "stable".into(),
        LineSupport::ActiveLts | LineSupport::Maintenance | LineSupport::Eol if lts => {
            match codename {
                Some(codename) => format!("{id} LTS ({codename})"),
                None => format!("{id} LTS"),
            }
        }
        _ => id.to_string(),
    }
}

fn find_line<'a>(lines: &'a [ReleaseLine], user_id: &str, key: LineKey) -> Option<&'a ReleaseLine> {
    if key == LineKey::Single {
        return lines
            .iter()
            .find(|line| line.id == "stable")
            .or_else(|| lines.first());
    }
    lines.iter().find(|line| line.id == user_id)
}

fn newer_than(user_id: &str, lines: &[ReleaseLine]) -> Vec<NewerLine> {
    let mut newer = lines
        .iter()
        .filter(|line| {
            line.support != LineSupport::Eol && cmp_line_id(&line.id, user_id) == Ordering::Greater
        })
        .collect::<Vec<_>>();
    newer.sort_by(|left, right| cmp_line_id(&left.id, &right.id));
    newer
        .into_iter()
        .map(|line| NewerLine {
            label: line.label.clone(),
            latest: line.latest.clone(),
        })
        .collect()
}

fn unknown() -> LineJudgement {
    LineJudgement {
        line: String::new(),
        latest_version: String::new(),
        status: VersionStatus::Unknown,
        newer: Vec::new(),
    }
}

fn version_line_id(version: &str, key: LineKey) -> Option<String> {
    match key {
        LineKey::Single => Some("stable".into()),
        LineKey::Major => component(version, 0).map(|major| major.to_string()),
        LineKey::MajorMinor => {
            let major = component(version, 0)?;
            let minor = component(version, 1)?;
            Some(format!("{major}.{minor}"))
        }
    }
}

fn component(version: &str, index: usize) -> Option<u64> {
    let token = version_token(version)?;
    token.split('.').nth(index)?.parse().ok()
}

fn cmp_line_id(left: &str, right: &str) -> Ordering {
    line_parts(left).cmp(&line_parts(right))
}

fn line_parts(id: &str) -> (u64, u64) {
    let mut parts = id.split('.');
    let major = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|part| part.parse().ok()).unwrap_or(0);
    (major, minor)
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (to_semver(candidate), to_semver(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

fn is_eol(value: Option<&Value>, today: &str) -> bool {
    match value {
        Some(Value::Bool(eol)) => *eol,
        Some(Value::String(date)) => iso_date(date).is_some_and(|date| date < today),
        _ => false,
    }
}

fn lts_started(value: Option<&Value>, today: &str) -> bool {
    match value {
        Some(Value::Bool(started)) => *started,
        Some(Value::String(date)) => iso_date(date).is_some_and(|date| date <= today),
        _ => false,
    }
}

fn support_is_active(value: Option<&Value>, today: &str) -> bool {
    match value {
        Some(Value::String(date)) => iso_date(date).is_some_and(|date| date >= today),
        _ => true,
    }
}

fn iso_date(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    (bytes.len() >= 10 && bytes[4] == b'-' && bytes[7] == b'-').then_some(&value[..10])
}

fn finish(mut lines: Vec<ReleaseLine>) -> Vec<ReleaseLine> {
    lines.sort_by(|left, right| cmp_line_id(&left.id, &right.id));
    lines
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year as i32, month as u32, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-10-26";

    #[test]
    fn unix_epoch_is_1970_01_01() {
        assert_eq!(today_iso(0), "1970-01-01");
        assert_eq!(today_iso(86_400), "1970-01-02");
    }

    #[test]
    fn node_22_is_compared_inside_its_lts_line() {
        let lines = lines_from_node(
            r#"[
                {"version":"v26.12.1","lts":false},
                {"version":"v24.11.0","lts":"Krypton"},
                {"version":"v22.21.0","lts":"Jod"},
                {"version":"v22.14.0","lts":"Jod"},
                {"version":"v23.0.0-rc.1","lts":false}
            ]"#,
            Some(
                r#"[
                    {"cycle":"26","latest":"26.12.0","eol":"2029-04-30","lts":"2026-10-28","support":"2027-10-27"},
                    {"cycle":"24","latest":"24.11.0","eol":"2028-04-30","lts":"2025-10-28","support":"2026-10-20"},
                    {"cycle":"22","latest":"22.20.0","eol":"2027-04-30","lts":"2024-10-29","support":"2025-10-21"},
                    {"cycle":"20","latest":"20.19.0","eol":"2026-04-30","lts":"2023-10-24","support":"2024-10-22"}
                ]"#,
            ),
            TODAY,
        )
        .unwrap();
        let node_22 = lines.iter().find(|line| line.id == "22").unwrap();
        assert_eq!(node_22.latest, "22.21.0");
        assert_eq!(node_22.label, "22 LTS (Jod)");
        assert_eq!(
            lines.iter().find(|line| line.id == "26").unwrap().label,
            "26 Current"
        );
        assert_eq!(
            lines.iter().find(|line| line.id == "20").unwrap().support,
            LineSupport::Eol
        );

        let judged = judge_lines("22.14.0", &lines, LineKey::Major);
        assert_eq!(judged.status, VersionStatus::Outdated);
        assert_eq!(judged.line, "22 LTS (Jod)");
        assert_eq!(judged.latest_version, "22.21.0");
        assert_eq!(
            judged
                .newer
                .iter()
                .map(|line| line.label.as_str())
                .collect::<Vec<_>>(),
            vec!["24 LTS (Krypton)", "26 Current"]
        );

        let current = judge_lines("22.21.0", &lines, LineKey::Major);
        assert_eq!(current.status, VersionStatus::UpToDate);
        assert_eq!(current.newer.len(), 2);

        let eol = judge_lines("20.18.0", &lines, LineKey::Major);
        assert_eq!(eol.status, VersionStatus::Unsupported);
    }

    #[test]
    fn python_eol_line_lists_newer_supported_cycles() {
        let lines = lines_from_endoflife(
            r#"[
                {"cycle":"3.15","latest":"3.15.2","eol":"2031-10-31","support":"2028-10-09","lts":false},
                {"cycle":"3.14","latest":"3.14.10","eol":"2030-10-31","support":"2027-10-01","lts":false},
                {"cycle":"3.9","latest":"3.9.25","eol":"2025-10-31","lts":false}
            ]"#,
            TODAY,
        )
        .unwrap();
        let judged = judge_lines("3.9.6", &lines, LineKey::MajorMinor);
        assert_eq!(judged.status, VersionStatus::Unsupported);
        assert_eq!(judged.latest_version, "3.9.25");
        assert_eq!(
            judged
                .newer
                .iter()
                .map(|line| line.latest.as_str())
                .collect::<Vec<_>>(),
            vec!["3.14.10", "3.15.2"]
        );
        assert_eq!(
            lines.iter().find(|line| line.id == "3.15").unwrap().label,
            "3.15 Current"
        );
    }

    #[test]
    fn java_lts_does_not_become_outdated_because_a_newer_major_exists() {
        let lines = lines_from_endoflife(
            r#"[
                {"cycle":"27","latest":"27.0.1+8","eol":"2027-03-23","lts":false},
                {"cycle":"25","latest":"25.0.5+8","eol":"2031-09-30","lts":true},
                {"cycle":"21","latest":"21.0.8+9","eol":"2029-09-30","lts":true},
                {"cycle":"17","latest":"17.0.16+8","eol":"2027-10-31","lts":true},
                {"cycle":"11","latest":"11.0.28+6","eol":"2024-10-31","lts":true}
            ]"#,
            TODAY,
        )
        .unwrap();
        let judged = judge_lines("17.0.16", &lines, LineKey::Major);
        assert_eq!(judged.status, VersionStatus::UpToDate);
        assert_eq!(judged.line, "17 LTS");
        assert_eq!(
            judged
                .newer
                .iter()
                .map(|line| line.label.as_str())
                .collect::<Vec<_>>(),
            vec!["21 LTS", "25 LTS", "27 Current"]
        );
        assert_eq!(
            judge_lines("11.0.28", &lines, LineKey::Major).status,
            VersionStatus::Unsupported
        );
    }

    #[test]
    fn go_minor_outside_the_supported_set_is_unsupported() {
        let lines = lines_from_go(
            r#"[{"version":"go1.27.3","stable":true},{"version":"go1.26.10","stable":true},{"version":"go1.28rc1","stable":false}]"#,
        )
        .unwrap();
        let judged = judge_lines("1.26.1", &lines, LineKey::MajorMinor);
        assert_eq!(judged.status, VersionStatus::Outdated);
        assert_eq!(judged.latest_version, "1.26.10");
        assert_eq!(judged.newer.len(), 1);
        assert_eq!(
            judge_lines("1.25.0", &lines, LineKey::MajorMinor).status,
            VersionStatus::Unsupported
        );
    }

    #[test]
    fn a_single_release_line_has_no_newer_column() {
        let lines = single_line("2.56.0");
        let judged = judge_lines("2.50.1", &lines, LineKey::Single);
        assert_eq!(judged.status, VersionStatus::Outdated);
        assert_eq!(judged.line, "stable");
        assert!(judged.newer.is_empty());
    }
}
