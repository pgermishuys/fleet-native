//! `lib/analytics-sections.ts`: the views of Analytics as the analytics-menu lists them. Each has its own address.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Overview,
    Daily,
    Projects,
    Models,
    Sessions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Spend,
    Breakdown,
}

impl Section {
    /// The id the address carries.
    pub fn id(self) -> &'static str {
        match self {
            Section::Overview => "overview",
            Section::Daily => "daily",
            Section::Projects => "projects",
            Section::Models => "models",
            Section::Sessions => "sessions",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Section::Overview => "Overview",
            Section::Daily => "Daily",
            Section::Projects => "Projects",
            Section::Models => "Models",
            Section::Sessions => "Sessions",
        }
    }

    pub fn group(self) -> Group {
        match self {
            Section::Overview | Section::Daily => Group::Spend,
            _ => Group::Breakdown,
        }
    }
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Spend => "Spend",
            Group::Breakdown => "Breakdown",
        }
    }
}

/// `ANALYTICS_SECTIONS`, in the menu's order.
pub const SECTIONS: [Section; 5] = [Section::Overview, Section::Daily, Section::Projects, Section::Models, Section::Sessions];

/// `ANALYTICS_GROUPS`.
pub const GROUPS: [Group; 2] = [Group::Spend, Group::Breakdown];

/// `isAnalyticsSectionId`.
pub fn is_section_id(value: &str) -> bool {
    SECTIONS.iter().any(|s| s.id() == value)
}

/// `analyticsSectionPath`: Overview is the bare `/analytics`.
pub fn section_path(section: Section) -> String {
    if section == Section::Overview { "/analytics".to_string() } else { format!("/analytics/{}", section.id()) }
}

/// `analyticsSectionFromPath`: the section an address shows. The third segment after the trailing slashes are dropped
/// decides; anything that isn't a section (a stale link) shows Overview.
pub fn section_from_path(pathname: &str) -> Section {
    let segment = pathname.trim_end_matches('/').split('/').nth(2).unwrap_or("");
    SECTIONS.iter().copied().find(|s| s.id() == segment).unwrap_or(Section::Overview)
}
