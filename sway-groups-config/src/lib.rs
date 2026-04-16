use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
#[derive(Default)]
pub struct SwaygConfig {
    pub defaults: DefaultsConfig,
    pub bar: BarConfig,
    /// Assignment rules: when the daemon sees a new workspace whose name
    /// matches a rule, it assigns the workspace to the specified groups
    /// and/or marks it global — instead of adding it to the active group.
    #[serde(default)]
    pub assign: Vec<AssignRule>,
}

/// Controls how [`AssignRule::match_pattern`] is interpreted.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    #[default]
    Exact,
    Regex,
}

/// A rule that controls which groups a newly created workspace is added to.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssignRule {
    /// Pattern to match against the workspace name.
    #[serde(rename = "match")]
    pub match_pattern: String,
    /// How to interpret `match_pattern`. Default: `"exact"`.
    #[serde(default)]
    pub match_type: MatchType,
    /// Groups to add the workspace to. If non-empty, replaces the default
    /// "add to active group" behaviour.
    #[serde(default)]
    pub groups: Vec<String>,
    /// Whether to mark the workspace as global.
    #[serde(default)]
    pub global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DefaultsConfig {
    pub default_group: String,
    pub default_workspace: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BarConfig {
    pub workspaces: BarSectionConfig,
    pub groups: BarSectionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct BarSectionConfig {
    pub socket_instance: String,
    pub display: BarDisplay,
    pub show_global: bool,
    pub show_empty: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BarDisplay {
    All,
    Active,
    None,
}


impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            default_group: "0".to_string(),
            default_workspace: "0".to_string(),
        }
    }
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            workspaces: BarSectionConfig {
                socket_instance: "swayg_workspaces".to_string(),
                display: BarDisplay::All,
                show_global: true,
                show_empty: true,
            },
            groups: BarSectionConfig {
                socket_instance: "swayg_groups".to_string(),
                display: BarDisplay::All,
                show_global: true,
                show_empty: true,
            },
        }
    }
}

impl Default for BarSectionConfig {
    fn default() -> Self {
        Self {
            socket_instance: String::new(),
            display: BarDisplay::All,
            show_global: true,
            show_empty: true,
        }
    }
}

impl SwaygConfig {
    /// Return all assignment rules whose pattern matches the given workspace name.
    pub fn matching_rules(&self, ws_name: &str) -> Vec<&AssignRule> {
        self.assign
            .iter()
            .filter(|rule| match rule.match_type {
                MatchType::Exact => rule.match_pattern == ws_name,
                MatchType::Regex => regex::Regex::new(&rule.match_pattern)
                    .map(|re| re.is_match(ws_name))
                    .unwrap_or(false),
            })
            .collect()
    }

    pub fn config_path() -> Option<PathBuf> {
        let dirs = directories::ProjectDirs::from("com", "swayg", "swayg")?;
        Some(dirs.config_dir().join("config.toml"))
    }

    pub fn load() -> anyhow::Result<Self> {
        if let Ok(env_path) = std::env::var("SWAYG_CONFIG") {
            return Self::load_from(std::path::Path::new(&env_path));
        }
        let path = Self::config_path()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &std::path::Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: SwaygConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn dump(&self) -> anyhow::Result<String> {
        let mut output = String::new();
        output.push_str("# swayg configuration\n");
        output.push_str("# Place at: ~/.config/swayg/config.toml\n\n");
        output.push_str(&toml::to_string_pretty(self)?);
        Ok(output)
    }

    pub fn dump_to(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = self.dump()?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
