//! Invented Analytics answers for scenes and tests with no server (component 20, piece B): six projects over 30 days, five
//! models on three providers, 18 sessions. The summary lists the top five projects and models by cost, as the server does.
use crate::fleet::analytics::store::MockAnswers;
use crate::fleet::analytics::{Daily, ModelRow, SessionRow, Summary, TopItem};

const PROJECTS: [(&str, f64, f64); 5] = [
    ("harbor-api", 18_400_000.0, 41.30),
    ("kettle-web", 12_900_000.0, 27.85),
    ("lantern-docs", 7_200_000.0, 14.10),
    ("orchard-cli", 4_600_000.0, 9.40),
    ("meadow-infra", 2_100_000.0, 4.55),
];

/// (id, provider, tokens, cost, messages)
const MODELS: [(&str, &str, f64, f64, f64); 5] = [
    ("claude-opus-5-5", "anthropic", 14_800_000.0, 52.40, 410.0),
    ("claude-sonnet-5-5", "anthropic", 19_300_000.0, 31.20, 880.0),
    ("gpt-5.1", "openai", 6_900_000.0, 8.75, 260.0),
    ("gemini-3-pro", "google", 3_100_000.0, 3.10, 120.0),
    ("claude-haiku-5-5", "anthropic", 900_000.0, 0.45, 95.0),
];

fn top(items: &[(&str, f64, f64)]) -> Vec<TopItem> {
    items.iter().map(|(name, tokens, cost)| TopItem { name: name.to_string(), tokens: *tokens, cost: *cost }).collect()
}

pub fn summary() -> Summary {
    let models: Vec<(&str, f64, f64)> = MODELS.iter().map(|(id, _, tokens, cost, _)| (*id, *tokens, *cost)).collect();
    Summary {
        total_tokens: 45_000_000.0,
        total_cost: 96.90,
        total_estimated_cost: 101.20,
        session_count: 18.0,
        message_count: 1765.0,
        top_models: top(&models),
        top_projects: top(&PROJECTS),
    }
}

/// Days that had events, oldest first, ending on 20 November 2026 (the day the pictures are drawn for).
pub fn daily() -> Vec<Daily> {
    const COST: [f64; 12] = [3.2, 5.8, 2.1, 7.4, 9.9, 4.3, 6.6, 11.2, 8.0, 5.1, 7.7, 13.6];
    COST.iter()
        .enumerate()
        .map(|(i, cost)| Daily {
            date: crate::fleet::summary::iso_day(20_777 - (COST.len() - 1 - i) as i64 * 2),
            tokens: cost * 480_000.0,
            cost: *cost,
            estimated_cost: cost * 1.04,
            sessions: 1.0 + (i % 4) as f64,
            messages: 40.0 + 11.0 * i as f64,
        })
        .collect()
}

pub fn models() -> Vec<ModelRow> {
    MODELS
        .iter()
        .map(|(id, provider, tokens, cost, messages)| ModelRow {
            model_id: id.to_string(),
            provider_id: provider.to_string(),
            tokens: *tokens,
            cost: *cost,
            estimated_cost: cost * 1.04,
            message_count: *messages,
            avg_cost_per_message: cost / messages,
        })
        .collect()
}

/// Eighteen sessions, newest first, as the server sends them.
pub fn sessions() -> Vec<SessionRow> {
    const TITLES: [&str; 6] = [
        "Fix the paging bug",
        "Tidy the changelog",
        "Add a retry to uploads",
        "Explain the cache",
        "Rename the flags",
        "Draft release notes",
    ];
    (0..18)
        .map(|i| {
            let (project, ..) = PROJECTS[i % PROJECTS.len()];
            let (model, ..) = MODELS[i % MODELS.len()];
            SessionRow {
                session_id: format!("s{}", i + 1),
                title: (i % 7 != 6).then(|| format!("{} {}", TITLES[i % TITLES.len()], i / TITLES.len() + 1)),
                project_id: Some(format!("p{}", i % PROJECTS.len())),
                project_name: Some(project.to_string()),
                tokens: 3_800_000.0 - 190_000.0 * i as f64,
                cost: 9.0 - 0.4 * i as f64,
                estimated_cost: 9.4 - 0.4 * i as f64,
                models: if i % 3 == 0 {
                    vec![model.to_string(), MODELS[(i + 1) % MODELS.len()].0.to_string()]
                } else {
                    vec![model.to_string()]
                },
                duration_seconds: (i % 5 != 4).then_some(600.0 + 540.0 * i as f64),
                created_at: format!("2026-11-{:02}T09:{:02}:00Z", 20 - i / 2, 10 + i),
            }
        })
        .collect()
}

/// The Analytics page for the pictures and tests: no server, so the reads are the invented answers above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    /// Overview open.
    Menu,
    /// Models open.
    Models,
    /// Another machine is listed, so the menu has its switch.
    Machines,
    /// Overview, the filters block shown.
    Filters,
    /// The project list open.
    ProjectOpen,
    /// From's calendar open on the month it holds.
    Calendar,
    /// From's calendar a month on: today ringed, the days after To dimmed.
    CalendarLimits,
    /// No read has answered yet (piece F).
    Loading,
    /// Every read failed with the server's 503.
    Failed,
    /// Every read answered with nothing.
    Nothing,
}

impl crate::State {
    /// A scene with another section open (Daily, Projects).
    pub fn show_analytics_section(&mut self, dark: bool, scene: Scene, section: crate::fleet::analytics::sections::Section) {
        self.show_analytics(dark, scene);
        self.open_analytics_section(section);
    }

    pub fn show_analytics(&mut self, dark: bool, scene: Scene) {
        use crate::fleet::analytics::sections::Section;
        self.dark = Some(dark);
        // The day the pictures are drawn for, so the range the filters show is the same every time.
        self.now_ms = Some(20_777 * 86_400_000 + 43_200_000);
        if scene == Scene::Machines {
            self.show_machines_only(super::machines::Case::Several);
        }
        self.enter_analytics();
        if scene == Scene::Models {
            self.open_analytics_section(Section::Models);
        }
        // The counts are the reads' lengths, so the scene holds the answers the first look would ask for.
        let a = &mut self.analytics;
        a.mock_answers = match scene {
            Scene::Loading => MockAnswers::Silent,
            Scene::Failed => MockAnswers::Failed,
            Scene::Nothing => MockAnswers::Empty,
            _ => MockAnswers::Data,
        };
        if a.mock_answers == MockAnswers::Data {
            a.summary.data = Some(summary());
            a.range.data = Some(summary());
            a.daily.data = daily();
            a.models.data = models();
            a.sessions.data = sessions();
        }
        if scene == Scene::ProjectOpen {
            self.ui.open = Some(crate::ui::Open::Select(crate::view::analytics_page::filters::PROJECT));
        }
        if matches!(scene, Scene::Calendar | Scene::CalendarLimits) {
            // The page mounts on its first look; the calendar opens on the range it will hold.
            self.analytics.filters = crate::fleet::analytics::filters::Filters::defaults(20_777 * 86_400_000 + 43_200_000);
            self.analytics_open_calendar(crate::fleet::analytics::calendar::End::From);
            if scene == Scene::CalendarLimits {
                self.analytics_calendar_shift(1);
            }
        }
        let a = &mut self.analytics;
        if a.mock_answers != MockAnswers::Data {
            // The page mounts on its first look and asks again: the answers come from there.
            return;
        }
        for loading in [
            &mut a.summary.is_loading,
            &mut a.range.is_loading,
            &mut a.daily.is_loading,
            &mut a.models.is_loading,
            &mut a.sessions.is_loading,
        ] {
            *loading = false;
        }
    }
}
