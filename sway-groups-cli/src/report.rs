//! What the read commands answer, before anyone decides how to print it.
//!
//! Every read command builds one of the types below and hands it to [`emit`],
//! which prints either the human text -- byte for byte what swayg has always
//! printed -- or JSON. The `Display` impls are therefore the specification of
//! the text form: nothing else in the CLI prints a listing.
//!
//! Presentation-only switches (`--plain`, `--groups`, `--flatten`) live in
//! [`ListStyle`] and are skipped when serializing: JSON always carries the
//! full shape and leaves the choosing to its reader. `--visible`, by contrast,
//! is a *filter* -- it changes which workspaces are answered, not how they
//! look -- so it has a report of its own.

use std::fmt;

use serde::Serialize;
use sway_groups_core::services::group_service::GroupInfo;
use sway_groups_core::services::workspace_service::WorkspaceInfo;

/// Print `report` as JSON or as its human text.
pub fn emit<R>(json: bool, report: &R) -> anyhow::Result<()>
where
    R: Serialize + fmt::Display,
{
    if json {
        println!("{}", serde_json::to_string_pretty(report)?);
    } else {
        // The `Display` impls end every line themselves; an empty report
        // prints nothing at all.
        print!("{report}");
    }
    Ok(())
}

/// `group list`.
#[derive(Debug, Serialize)]
pub struct GroupList {
    pub groups: Vec<Group>,
}

#[derive(Debug, Serialize)]
pub struct Group {
    pub name: String,
    pub workspaces: Vec<String>,
}

impl GroupList {
    pub fn new(groups: &[GroupInfo]) -> Self {
        Self {
            groups: groups
                .iter()
                .map(|group| Group {
                    name: group.name.clone(),
                    workspaces: group.workspaces.clone(),
                })
                .collect(),
        }
    }
}

impl fmt::Display for GroupList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.groups.is_empty() {
            return writeln!(f, "No groups found.");
        }
        for group in &self.groups {
            writeln!(f, "Group \"{}\":", group.name)?;
            if group.workspaces.is_empty() {
                writeln!(f, "  (empty)")?;
            } else {
                for ws in &group.workspaces {
                    writeln!(f, "  - {ws}")?;
                }
            }
        }
        Ok(())
    }
}

/// The text-only switches of `workspace list`.
#[derive(Debug, Clone, Copy, Default)]
pub struct ListStyle {
    pub plain: bool,
    pub groups: bool,
    pub flatten: bool,
}

/// `workspace list --visible`: names only, because that is what the filter is
/// asked for -- by bars and keybindings that feed the answer straight on.
#[derive(Debug, Serialize)]
pub struct VisibleWorkspaces {
    pub output: String,
    pub workspaces: Vec<String>,
    #[serde(skip)]
    pub plain: bool,
}

impl fmt::Display for VisibleWorkspaces {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.workspaces.is_empty() {
            if !self.plain {
                writeln!(f, "No visible workspaces found.")?;
            }
            return Ok(());
        }
        for ws in &self.workspaces {
            writeln!(f, "{ws}")?;
        }
        Ok(())
    }
}

/// Where a workspace stands relative to the active group of its output.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    /// Global workspaces show in every group.
    Global,
    Visible,
    Hidden,
    /// No active group to compare against -- the question was not asked (a
    /// `--group` filter) or could not be answered (no output).
    Unknown,
}

impl Visibility {
    /// `active` is the active group *if it was looked up at all*: the outer
    /// `None` means nobody asked, `Some(None)` means the output has none.
    fn of(ws: &WorkspaceInfo, active: Option<&Option<String>>) -> Self {
        if ws.is_global {
            return Self::Global;
        }
        let Some(active) = active else {
            return Self::Unknown;
        };
        if ws
            .groups
            .iter()
            .any(|g| Some(g.as_str()) == active.as_deref())
        {
            Self::Visible
        } else if ws.groups.is_empty() {
            // Belonging nowhere is not the same as being filed elsewhere.
            Self::Visible
        } else {
            Self::Hidden
        }
    }

    /// The parenthesised marker of the formatted list.
    fn label(self) -> &'static str {
        match self {
            Self::Global => "(global)",
            Self::Visible => "(visible)",
            Self::Hidden => "(hidden)",
            Self::Unknown => "",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Workspace {
    pub name: String,
    pub output: Option<String>,
    pub global: bool,
    pub groups: Vec<String>,
    pub visibility: Visibility,
}

impl Workspace {
    /// The workspace's groups with the active one first, as the flattened
    /// text form lists them.
    fn groups_active_first(&self, active: Option<&str>) -> Vec<&String> {
        let mut groups: Vec<&String> = self.groups.iter().collect();
        groups.sort_by(|a, b| {
            if let Some(active) = active {
                if active == a.as_str() {
                    return std::cmp::Ordering::Less;
                }
                if active == b.as_str() {
                    return std::cmp::Ordering::Greater;
                }
            }
            a.cmp(b)
        });
        groups
    }
}

/// `workspace list`.
#[derive(Debug, Serialize)]
pub struct WorkspaceList {
    /// The `--output` filter, as given.
    pub output: Option<String>,
    /// The `--group` filter, as given.
    pub group: Option<String>,
    /// The active group the visibilities were judged against, if one was
    /// looked up.
    pub active_group: Option<String>,
    pub workspaces: Vec<Workspace>,
    #[serde(skip)]
    pub style: ListStyle,
}

impl WorkspaceList {
    pub fn new(
        workspaces: &[WorkspaceInfo],
        output: Option<&str>,
        group: Option<&str>,
        active_group: Option<&Option<String>>,
        style: ListStyle,
    ) -> Self {
        Self {
            output: output.map(str::to_owned),
            group: group.map(str::to_owned),
            active_group: active_group.and_then(Clone::clone),
            workspaces: workspaces
                .iter()
                .map(|ws| Workspace {
                    name: ws.name.clone(),
                    output: ws.output.clone(),
                    global: ws.is_global,
                    groups: ws.groups.clone(),
                    visibility: Visibility::of(ws, active_group),
                })
                .collect(),
            style,
        }
    }
}

impl fmt::Display for WorkspaceList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.workspaces.is_empty() {
            if !self.style.plain {
                writeln!(f, "No workspaces found.")?;
            }
            return Ok(());
        }
        if !self.style.plain {
            writeln!(
                f,
                "Workspaces in group \"{}\" on \"{}\":",
                self.group.as_deref().unwrap_or("active"),
                self.output.as_deref().unwrap_or("all")
            )?;
        }
        let ListStyle {
            plain,
            groups,
            flatten,
        } = self.style;
        for ws in &self.workspaces {
            match (plain, groups, flatten) {
                (true, true, true) => {
                    for group in ws.groups_active_first(self.active_group.as_deref()) {
                        writeln!(f, "{}│{}", ws.name, group)?;
                    }
                }
                (true, true, false) => writeln!(f, "{}│{}", ws.name, ws.groups.join(","))?,
                (true, false, _) => writeln!(f, "{}", ws.name)?,
                (false, _, _) => writeln!(f, "  {:20} {}", ws.name, ws.visibility.label())?,
            }
        }
        Ok(())
    }
}

/// `workspace groups <workspace>`.
#[derive(Debug, Serialize)]
pub struct WorkspaceGroups {
    pub workspace: String,
    pub groups: Vec<String>,
}

impl fmt::Display for WorkspaceGroups {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.groups.is_empty() {
            return writeln!(f, "Workspace \"{}\" is not in any group.", self.workspace);
        }
        let groups = self
            .groups
            .iter()
            .map(|g| format!("\"{g}\""))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            f,
            "Workspace \"{}\" is in groups: {}",
            self.workspace, groups
        )
    }
}

/// `status`.
#[derive(Debug, Serialize)]
pub struct Status {
    pub show_hidden_workspaces: bool,
    pub outputs: Vec<OutputStatus>,
}

#[derive(Debug, Serialize)]
pub struct OutputStatus {
    pub name: String,
    pub active_group: Option<String>,
    pub visible: Vec<String>,
    /// On this output but filed in another group.
    pub inactive: Vec<String>,
    /// Hidden by the user within the active group.
    pub hidden: Vec<String>,
    pub global: Vec<String>,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "show_hidden_workspaces = {}",
            self.show_hidden_workspaces
        )?;
        for output in &self.outputs {
            writeln!(
                f,
                "{}: active group = \"{}\"",
                output.name,
                output.active_group.as_deref().unwrap_or("(none)")
            )?;
            writeln!(f, "  Visible:  {}", or_none(&output.visible))?;
            writeln!(f, "  Inactive: {}", or_none(&output.inactive))?;
            writeln!(f, "  Hidden:   {}", or_none(&output.hidden))?;
            if !output.global.is_empty() {
                writeln!(f, "  Global:   {}", output.global.join(", "))?;
            }
        }
        Ok(())
    }
}

fn or_none(names: &[String]) -> String {
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str, groups: &[&str], is_global: bool) -> WorkspaceInfo {
        WorkspaceInfo {
            id: 1,
            name: name.to_string(),
            number: None,
            output: Some("DP-1".to_string()),
            is_global,
            groups: groups.iter().map(|g| g.to_string()).collect(),
        }
    }

    fn group(name: &str, workspaces: &[&str]) -> GroupInfo {
        GroupInfo {
            id: 1,
            name: name.to_string(),
            workspaces: workspaces.iter().map(|w| w.to_string()).collect(),
        }
    }

    #[test]
    fn a_group_without_workspaces_says_so_rather_than_printing_nothing() {
        let text =
            GroupList::new(&[group("editing", &[]), group("reading", &["10_notes"])]).to_string();
        assert_eq!(
            text,
            "Group \"editing\":\n  (empty)\nGroup \"reading\":\n  - 10_notes\n"
        );
    }

    #[test]
    fn no_groups_at_all_is_a_sentence_not_an_empty_listing() {
        assert_eq!(GroupList::new(&[]).to_string(), "No groups found.\n");
    }

    #[test]
    fn the_formatted_list_marks_every_workspace_against_the_active_group() {
        let active = Some("editing".to_string());
        let report = WorkspaceList::new(
            &[
                workspace("10_notes", &["editing"], false),
                workspace("20_mail", &["reading"], false),
                workspace("30_chat", &[], true),
                workspace("40_scratch", &[], false),
            ],
            None,
            None,
            Some(&active),
            ListStyle::default(),
        );
        assert_eq!(
            report.to_string(),
            "Workspaces in group \"active\" on \"all\":\n\
             \x20 10_notes             (visible)\n\
             \x20 20_mail              (hidden)\n\
             \x20 30_chat              (global)\n\
             \x20 40_scratch           (visible)\n"
        );
    }

    #[test]
    fn without_an_active_group_the_formatted_list_leaves_the_marker_off() {
        let report = WorkspaceList::new(
            &[workspace("10_notes", &["editing"], false)],
            None,
            Some("editing"),
            None,
            ListStyle::default(),
        );
        assert_eq!(
            report.to_string(),
            "Workspaces in group \"editing\" on \"all\":\n  10_notes             \n"
        );
    }

    #[test]
    fn plain_drops_the_header_and_the_empty_case_falls_silent() {
        let style = ListStyle {
            plain: true,
            ..ListStyle::default()
        };
        let filled = WorkspaceList::new(
            &[workspace("10_notes", &["editing"], false)],
            None,
            None,
            None,
            style,
        );
        assert_eq!(filled.to_string(), "10_notes\n");

        let empty = WorkspaceList::new(&[], None, None, None, style);
        assert_eq!(empty.to_string(), "");
        assert_eq!(
            WorkspaceList::new(&[], None, None, None, ListStyle::default()).to_string(),
            "No workspaces found.\n"
        );
    }

    #[test]
    fn flattening_puts_the_active_group_first_and_sorts_the_rest() {
        let active = Some("editing".to_string());
        let report = WorkspaceList::new(
            &[workspace(
                "10_notes",
                &["reading", "archive", "editing"],
                false,
            )],
            None,
            None,
            Some(&active),
            ListStyle {
                plain: true,
                groups: true,
                flatten: true,
            },
        );
        assert_eq!(
            report.to_string(),
            "10_notes│editing\n10_notes│archive\n10_notes│reading\n"
        );
    }

    #[test]
    fn a_workspace_in_no_group_still_gets_its_separator() {
        let report = WorkspaceList::new(
            &[workspace("40_scratch", &[], false)],
            None,
            None,
            None,
            ListStyle {
                plain: true,
                groups: true,
                flatten: false,
            },
        );
        assert_eq!(report.to_string(), "40_scratch│\n");
    }

    #[test]
    fn visible_workspaces_stay_silent_when_plain_has_nothing_to_show() {
        let empty = VisibleWorkspaces {
            output: "DP-1".to_string(),
            workspaces: Vec::new(),
            plain: true,
        };
        assert_eq!(empty.to_string(), "");
        assert_eq!(
            VisibleWorkspaces {
                plain: false,
                ..empty
            }
            .to_string(),
            "No visible workspaces found.\n"
        );
    }

    #[test]
    fn a_workspaces_groups_are_quoted_and_comma_separated() {
        let report = WorkspaceGroups {
            workspace: "10_notes".to_string(),
            groups: vec!["editing".to_string(), "reading".to_string()],
        };
        assert_eq!(
            report.to_string(),
            "Workspace \"10_notes\" is in groups: \"editing\", \"reading\"\n"
        );
        assert_eq!(
            WorkspaceGroups {
                workspace: "40_scratch".to_string(),
                groups: Vec::new(),
            }
            .to_string(),
            "Workspace \"40_scratch\" is not in any group.\n"
        );
    }

    #[test]
    fn status_names_the_empty_lists_but_omits_globals_it_has_none_of() {
        let report = Status {
            show_hidden_workspaces: false,
            outputs: vec![
                OutputStatus {
                    name: "DP-1".to_string(),
                    active_group: Some("editing".to_string()),
                    visible: vec!["10_notes".to_string()],
                    inactive: vec!["20_mail".to_string()],
                    hidden: Vec::new(),
                    global: vec!["30_chat".to_string()],
                },
                OutputStatus {
                    name: "DP-2".to_string(),
                    active_group: None,
                    visible: Vec::new(),
                    inactive: Vec::new(),
                    hidden: Vec::new(),
                    global: Vec::new(),
                },
            ],
        };
        assert_eq!(
            report.to_string(),
            "show_hidden_workspaces = false\n\
             DP-1: active group = \"editing\"\n\
             \x20 Visible:  10_notes\n\
             \x20 Inactive: 20_mail\n\
             \x20 Hidden:   (none)\n\
             \x20 Global:   30_chat\n\
             DP-2: active group = \"(none)\"\n\
             \x20 Visible:  (none)\n\
             \x20 Inactive: (none)\n\
             \x20 Hidden:   (none)\n"
        );
    }
}
