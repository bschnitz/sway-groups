//! Folding workspace widgets into one per collapse rule.
//!
//! A group can hold dozens of similar workspaces (one per ticket, say), which
//! crowd the bar. A collapse rule names them by pattern and shows a single
//! widget in their place. This is purely visual: navigation, focus and the
//! DB never see the fold, and while the bar's modifier is held the daemon
//! sets `bar_expanded` so the bar shows every workspace again.

use regex::Regex;
use sway_groups_config::CollapseRule;
use tracing::warn;

use crate::sway::waybar_client::WidgetSpec;

/// The class carried by a widget that stands for several workspaces.
pub const COLLAPSED_CLASS: &str = "collapsed";

/// State classes a folded widget takes over from its members: it is focused
/// when one of them is, and so on.
const INHERITED_CLASSES: &[&str] = &["focused", "visible", "urgent"];

/// Collapse rules with their patterns compiled.
#[derive(Clone, Default)]
pub struct CollapseRules {
    rules: Vec<(Regex, String)>,
}

impl CollapseRules {
    /// Compile the configured rules. A rule whose pattern does not compile is
    /// logged and left out.
    pub fn new(rules: &[CollapseRule]) -> Self {
        let rules = rules
            .iter()
            .filter_map(|rule| match Regex::new(&rule.pattern) {
                Ok(re) => Some((re, rule.label.clone())),
                Err(e) => {
                    warn!("Ignoring collapse rule '{}': {}", rule.pattern, e);
                    None
                }
            })
            .collect();
        Self { rules }
    }

    /// Replace the widgets of every rule's workspaces by one widget labelled
    /// `<label> (<count>)`. The first matching rule wins; widgets no rule
    /// matches are returned unchanged.
    pub fn fold(&self, widgets: Vec<WidgetSpec>) -> Vec<WidgetSpec> {
        if self.rules.is_empty() {
            return widgets;
        }
        let mut members: Vec<Vec<WidgetSpec>> = vec![Vec::new(); self.rules.len()];
        let mut out = Vec::new();
        for widget in widgets {
            match self
                .rules
                .iter()
                .position(|(re, _)| re.is_match(&widget.label))
            {
                Some(i) => members[i].push(widget),
                None => out.push(widget),
            }
        }
        for ((_, label), mut group) in self.rules.iter().zip(members) {
            if group.is_empty() {
                continue;
            }
            group.sort_by(|a, b| a.label.cmp(&b.label));
            out.push(Self::folded(label, &group));
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
            label: format!("{} ({})", label, group.len()),
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

    fn rules(rules: &[(&str, &str)]) -> CollapseRules {
        CollapseRules::new(
            &rules
                .iter()
                .map(|(pattern, label)| CollapseRule {
                    pattern: pattern.to_string(),
                    label: label.to_string(),
                })
                .collect::<Vec<_>>(),
        )
    }

    fn labels(widgets: &[WidgetSpec]) -> Vec<&str> {
        widgets.iter().map(|w| w.label.as_str()).collect()
    }

    #[test]
    fn matching_workspaces_become_one_widget_with_a_count() {
        let folded = rules(&[("^20_(ALPHA|BETA)", "20_Tickets")]).fold(vec![
            widget("10_mail", &[]),
            widget("20_ALPHA-1", &[]),
            widget("20_BETA-7", &[]),
            widget("20_ALPHA-2", &[]),
        ]);
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
        let folded = rules(&[("^20_", "20_Tickets")]).fold(vec![
            widget("20_a", &["urgent"]),
            widget("20_b", &["focused", "hidden"]),
            widget("20_c", &["global"]),
        ]);
        assert_eq!(folded[0].classes, ["collapsed", "focused", "urgent"]);
        assert_eq!(folded[0].on_click.as_deref(), Some("go 20_b"));
    }

    #[test]
    fn the_first_matching_rule_wins() {
        let folded = rules(&[("^20_a", "first"), ("^20_", "second")])
            .fold(vec![widget("20_a", &[]), widget("20_b", &[])]);
        assert_eq!(labels(&folded), ["first (1)", "second (1)"]);
    }

    #[test]
    fn a_rule_without_members_adds_nothing() {
        let folded = rules(&[("^99_", "nothing")]).fold(vec![widget("10_mail", &[])]);
        assert_eq!(labels(&folded), ["10_mail"]);
    }

    #[test]
    fn a_rule_that_does_not_compile_is_left_out() {
        let folded =
            rules(&[("(", "broken"), ("^20_", "20_Tickets")]).fold(vec![widget("20_a", &[])]);
        assert_eq!(labels(&folded), ["20_Tickets (1)"]);
    }
}
