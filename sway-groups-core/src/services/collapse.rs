//! Folding workspaces into one per collapse rule.
//!
//! A group can hold dozens of similar workspaces (one per ticket, say), which
//! crowd the bar. A collapse rule names them by pattern and shows a single
//! widget in their place. While the bar's modifier is held the daemon sets
//! `bar_expanded`, and the bar shows the workspaces of every rule with
//! `expand_on_modifier` one by one again.
//!
//! Navigation sees the fold only for a rule without `step_through`: `nav
//! next`/`nav prev` then stop once on it, at its place in the bar, and go to
//! the workspace of it focused last (the daemon remembers it per rule).

use regex::Regex;
use sea_orm::DatabaseConnection;
use sway_groups_config::CollapseRule;
use tracing::warn;

use crate::db::entities::setting;
use crate::db::queries::{get_setting, set_setting};
use crate::error::Result;
use crate::sway::waybar_client::WidgetSpec;

/// The class carried by a widget that stands for several workspaces.
pub const COLLAPSED_CLASS: &str = "collapsed";

/// State classes a folded widget takes over from its members: it is focused
/// when one of them is, and so on.
const INHERITED_CLASSES: &[&str] = &["focused", "visible", "urgent"];

#[derive(Clone)]
struct Rule {
    re: Regex,
    label: String,
    expand_on_modifier: bool,
    step_through: bool,
}

/// One stop of `nav next`/`nav prev`: a single workspace, or the workspaces
/// of a fold that is not stepped through.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavStop {
    /// The rule's label when the stop is a fold.
    pub fold: Option<String>,
    /// The workspaces of the stop, sorted.
    pub workspaces: Vec<String>,
}

/// Collapse rules with their patterns compiled.
#[derive(Clone, Default)]
pub struct CollapseRules {
    rules: Vec<Rule>,
}

impl CollapseRules {
    /// Compile the configured rules. A rule whose pattern does not compile is
    /// logged and left out.
    pub fn new(rules: &[CollapseRule]) -> Self {
        let rules = rules
            .iter()
            .filter_map(|rule| match Regex::new(&rule.pattern) {
                Ok(re) => Some(Rule {
                    re,
                    label: rule.label.clone(),
                    expand_on_modifier: rule.expand_on_modifier,
                    step_through: rule.step_through,
                }),
                Err(e) => {
                    warn!("Ignoring collapse rule '{}': {}", rule.pattern, e);
                    None
                }
            })
            .collect();
        Self { rules }
    }

    /// Index of the first rule matching `workspace`.
    fn rule_of(&self, workspace: &str) -> Option<usize> {
        self.rules.iter().position(|r| r.re.is_match(workspace))
    }

    /// Whether a rule matches `workspace`.
    pub fn matches(&self, workspace: &str) -> bool {
        self.rule_of(workspace).is_some()
    }

    /// Replace the widgets of every rule's workspaces by one widget labelled
    /// `<label> (<count>)`. The first matching rule wins; widgets no rule
    /// matches are returned unchanged. With `expanded` only the rules without
    /// `expand_on_modifier` fold.
    pub fn fold(&self, widgets: Vec<WidgetSpec>, expanded: bool) -> Vec<WidgetSpec> {
        if self.rules.is_empty() {
            return widgets;
        }
        let mut members: Vec<Vec<WidgetSpec>> = vec![Vec::new(); self.rules.len()];
        let mut out = Vec::new();
        for widget in widgets {
            match self.rule_of(&widget.label) {
                Some(i) if !expanded || !self.rules[i].expand_on_modifier => {
                    members[i].push(widget)
                }
                _ => out.push(widget),
            }
        }
        for (rule, mut group) in self.rules.iter().zip(members) {
            if group.is_empty() {
                continue;
            }
            group.sort_by(|a, b| a.label.cmp(&b.label));
            out.push(Self::folded(&rule.label, &group));
        }
        out
    }

    fn folded(label: &str, group: &[WidgetSpec]) -> WidgetSpec {
        let mut classes = vec![COLLAPSED_CLASS.to_string()];
        for class in INHERITED_CLASSES {
            if group.iter().any(|w| w.classes.iter().any(|c| c == class)) {
                classes.push(class.to_string());
            }
        }
        // A click goes where it would have gone on the focused member, or on
        // the first one.
        let target = group
            .iter()
            .find(|w| w.classes.iter().any(|c| c == "focused"))
            .unwrap_or(&group[0]);
        WidgetSpec {
            id: format!("collapse-{}", label),
            label: folded_label(label, group.len()),
            classes,
            tooltip: Some(
                group
                    .iter()
                    .map(|w| w.label.as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
            on_click: target.on_click.clone(),
            on_right_click: None,
            on_middle_click: None,
        }
    }

    /// The stops `nav next`/`nav prev` go through, in the order of the bar:
    /// the workspaces of a rule without `step_through` make one stop, sorted
    /// by the fold's label, every other workspace a stop of its own.
    pub fn nav_stops(&self, workspaces: &[String]) -> Vec<NavStop> {
        let mut members: Vec<Vec<String>> = vec![Vec::new(); self.rules.len()];
        let mut stops = Vec::new();
        for ws in workspaces {
            match self.rule_of(ws) {
                Some(i) if !self.rules[i].step_through => members[i].push(ws.clone()),
                _ => stops.push((
                    ws.clone(),
                    NavStop {
                        fold: None,
                        workspaces: vec![ws.clone()],
                    },
                )),
            }
        }
        for (rule, mut group) in self.rules.iter().zip(members) {
            if group.is_empty() {
                continue;
            }
            group.sort();
            stops.push((
                folded_label(&rule.label, group.len()),
                NavStop {
                    fold: Some(rule.label.clone()),
                    workspaces: group,
                },
            ));
        }
        stops.sort_by(|a, b| a.0.cmp(&b.0));
        stops.into_iter().map(|(_, stop)| stop).collect()
    }

    /// Remember `workspace` as the one focused last of the fold it belongs
    /// to; a workspace no rule matches is ignored.
    pub async fn remember(&self, conn: &DatabaseConnection, workspace: &str) -> Result<()> {
        let Some(i) = self.rule_of(workspace) else {
            return Ok(());
        };
        set_setting(
            conn,
            &setting::collapse_last_key(&self.rules[i].label),
            workspace,
        )
        .await
    }

    /// The workspace to go to on `stop`: the one of a fold focused last, or
    /// its first one.
    pub async fn target(&self, conn: &DatabaseConnection, stop: &NavStop) -> Result<String> {
        if let Some(label) = &stop.fold {
            let last = get_setting(conn, &setting::collapse_last_key(label)).await?;
            if let Some(last) = last.filter(|l| stop.workspaces.contains(l)) {
                return Ok(last);
            }
        }
        Ok(stop.workspaces[0].clone())
    }
}

fn folded_label(label: &str, count: usize) -> String {
    format!("{} ({})", label, count)
}

/// The stop after (`forward`) or before the one holding `current`; `wrap`
/// goes round at the ends.
pub fn neighbour<'a>(
    stops: &'a [NavStop],
    current: &str,
    wrap: bool,
    forward: bool,
) -> Option<&'a NavStop> {
    let i = stops
        .iter()
        .position(|s| s.workspaces.iter().any(|w| w == current))?;
    let j = match (forward, wrap) {
        (true, _) if i + 1 < stops.len() => i + 1,
        (true, true) => 0,
        (false, _) if i > 0 => i - 1,
        (false, true) => stops.len() - 1,
        _ => return None,
    };
    stops.get(j)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widget(name: &str, classes: &[&str]) -> WidgetSpec {
        WidgetSpec {
            id: format!("ws-{}", name),
            label: name.to_string(),
            classes: classes.iter().map(|c| c.to_string()).collect(),
            tooltip: None,
            on_click: Some(format!("go {}", name)),
            on_right_click: None,
            on_middle_click: None,
        }
    }

    fn rule(pattern: &str, label: &str) -> CollapseRule {
        CollapseRule {
            pattern: pattern.to_string(),
            label: label.to_string(),
            expand_on_modifier: true,
            step_through: true,
        }
    }

    fn rules(rules: &[(&str, &str)]) -> CollapseRules {
        CollapseRules::new(
            &rules
                .iter()
                .map(|(pattern, label)| rule(pattern, label))
                .collect::<Vec<_>>(),
        )
    }

    fn labels(widgets: &[WidgetSpec]) -> Vec<&str> {
        widgets.iter().map(|w| w.label.as_str()).collect()
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn matching_workspaces_become_one_widget_with_a_count() {
        let folded = rules(&[("^20_(ALPHA|BETA)", "20_Tickets")]).fold(
            vec![
                widget("10_mail", &[]),
                widget("20_ALPHA-1", &[]),
                widget("20_BETA-7", &[]),
                widget("20_ALPHA-2", &[]),
            ],
            false,
        );
        assert_eq!(labels(&folded), ["10_mail", "20_Tickets (3)"]);
        let fold = &folded[1];
        assert_eq!(fold.classes, ["collapsed"]);
        assert_eq!(
            fold.tooltip.as_deref(),
            Some("20_ALPHA-1\n20_ALPHA-2\n20_BETA-7")
        );
        assert_eq!(fold.on_click.as_deref(), Some("go 20_ALPHA-1"));
    }

    #[test]
    fn the_fold_takes_over_the_state_of_its_members() {
        let folded = rules(&[("^20_", "20_Tickets")]).fold(
            vec![
                widget("20_a", &["urgent"]),
                widget("20_b", &["focused", "hidden"]),
                widget("20_c", &["global"]),
            ],
            false,
        );
        assert_eq!(folded[0].classes, ["collapsed", "focused", "urgent"]);
        assert_eq!(folded[0].on_click.as_deref(), Some("go 20_b"));
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let folded = rules(&[("^20_a", "first"), ("^20_", "second")])
            .fold(vec![widget("20_a", &[]), widget("20_b", &[])], false);
        assert_eq!(labels(&folded), ["first (1)", "second (1)"]);
    }

    #[test]
    fn a_rule_without_members_adds_nothing() {
        let folded = rules(&[("^99_", "nothing")]).fold(vec![widget("10_mail", &[])], false);
        assert_eq!(labels(&folded), ["10_mail"]);
    }

    #[test]
    fn a_rule_that_does_not_compile_is_left_out() {
        let folded = rules(&[("(", "broken"), ("^20_", "20_Tickets")])
            .fold(vec![widget("20_a", &[])], false);
        assert_eq!(labels(&folded), ["20_Tickets (1)"]);
    }

    #[test]
    fn expanded_unfolds_only_the_rules_that_expand_on_the_modifier() {
        let mut fixed = rule("^30_", "30_fixed");
        fixed.expand_on_modifier = false;
        let rules = CollapseRules::new(&[rule("^20_", "20_open"), fixed]);
        let folded = rules.fold(
            vec![
                widget("20_a", &[]),
                widget("20_b", &[]),
                widget("30_a", &[]),
                widget("30_b", &[]),
            ],
            true,
        );
        assert_eq!(labels(&folded), ["20_a", "20_b", "30_fixed (2)"]);
    }

    #[test]
    fn a_fold_not_stepped_through_is_one_stop_at_its_place_in_the_bar() {
        let mut fold = rule("^20_(ALPHA|BETA)", "20_Tickets");
        fold.step_through = false;
        let stops = CollapseRules::new(&[fold]).nav_stops(&names(&[
            "10_mail",
            "20_ALPHA-1",
            "20_BETA-7",
            "20_Zebra",
            "20_ALPHA-2",
        ]));
        let fold = NavStop {
            fold: Some("20_Tickets".to_string()),
            workspaces: names(&["20_ALPHA-1", "20_ALPHA-2", "20_BETA-7"]),
        };
        assert_eq!(stops.len(), 3);
        assert_eq!(stops[0].workspaces, ["10_mail"]);
        assert_eq!(stops[1], fold);
        assert_eq!(stops[2].workspaces, ["20_Zebra"]);
    }

    #[test]
    fn a_fold_that_is_stepped_through_leaves_the_stops_alone() {
        let stops = rules(&[("^20_", "20_Tickets")]).nav_stops(&names(&["20_b", "10_a", "20_a"]));
        let order: Vec<&str> = stops.iter().map(|s| s.workspaces[0].as_str()).collect();
        assert_eq!(order, ["10_a", "20_a", "20_b"]);
        assert!(stops.iter().all(|s| s.fold.is_none()));
    }

    #[test]
    fn the_neighbour_of_a_member_is_the_neighbour_of_its_fold() {
        let mut fold = rule("^20_", "20_Tickets");
        fold.step_through = false;
        let stops =
            CollapseRules::new(&[fold]).nav_stops(&names(&["10_a", "20_x", "20_y", "30_b"]));
        let next = neighbour(&stops, "20_x", false, true).expect("a stop after the fold");
        assert_eq!(next.workspaces, ["30_b"]);
        let prev = neighbour(&stops, "20_y", false, false).expect("a stop before the fold");
        assert_eq!(prev.workspaces, ["10_a"]);
        assert_eq!(neighbour(&stops, "30_b", false, true), None);
        let wrapped = neighbour(&stops, "30_b", true, true).expect("wraps to the start");
        assert_eq!(wrapped.workspaces, ["10_a"]);
    }
}
