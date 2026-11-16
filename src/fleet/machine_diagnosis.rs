//! Why a machine isn't answering, as home worked it out (`lib/machine-diagnosis.ts`): which of six steps broke, and what to
//! try. Home keeps the last look at each machine (`diagnosis` in `GET /api/machines`) and looks again on
//! `POST /api/machines/{id}/check`. The mixed-content case (an https page calling an http address) can't happen in a native
//! app, so it isn't ported.
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Ok,
    Failed,
    /// The steps after the one that broke.
    NotChecked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub state: StepState,
    pub detail: Option<String>,
}

/// `MachineDiagnosis`: `code` is stable, the rest is words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnosis {
    pub code: String,
    pub headline: String,
    pub steps: Vec<Step>,
    pub next: String,
}

const STEP_LABELS: [&str; 6] = ["Name found", "Machine answers", "Port answers", "It's Fleet", "Version fits", "Key accepted"];

impl Diagnosis {
    /// None unless it has a headline and a way forward: a half-filled one would draw an empty tooltip.
    pub fn from_json(v: &Value) -> Option<Diagnosis> {
        let text = |x: &Value, k: &str| x.get(k).and_then(Value::as_str).map(str::to_string);
        let headline = text(v, "headline").filter(|h| !h.is_empty())?;
        let steps = v
            .get("steps")
            .and_then(Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|s| {
                        let state = match s.get("state")?.as_str()? {
                            "ok" => StepState::Ok,
                            "failed" => StepState::Failed,
                            "notChecked" => StepState::NotChecked,
                            _ => return None,
                        };
                        Some(Step { label: text(s, "label")?, state, detail: text(s, "detail").filter(|d| !d.is_empty()) })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Diagnosis { code: text(v, "code").unwrap_or_default(), headline, steps, next: text(v, "try").unwrap_or_default() })
    }

    /// The six steps with the first `index` ok, the one at `index` failed with `detail`, and the rest not checked.
    fn failing_at(index: usize, detail: &str) -> Vec<Step> {
        STEP_LABELS
            .iter()
            .enumerate()
            .map(|(at, label)| Step {
                label: (*label).to_string(),
                state: match at.cmp(&index) {
                    std::cmp::Ordering::Less => StepState::Ok,
                    std::cmp::Ordering::Equal => StepState::Failed,
                    std::cmp::Ordering::Greater => StepState::NotChecked,
                },
                detail: (at == index).then(|| detail.to_string()),
            })
            .collect()
    }

    /// `tunnelDownDiagnosis`: the headline for a saved SSH machine whose tunnel is down.
    pub fn tunnel_down(name: &str, destination: Option<&str>) -> Diagnosis {
        Diagnosis {
            code: "tunnel-down".into(),
            headline: format!("The SSH tunnel to {name} is down"),
            steps: Diagnosis::failing_at(1, "The tunnel Fleet keeps open isn't up."),
            next: format!(
                "Fleet opens it again by itself. If it stays down, check that ssh {} works in a terminal.",
                destination.unwrap_or(name)
            ),
        }
    }

    /// `diagnosisText`: the details as plain text, for pasting into a message to whoever helps.
    pub fn text(&self) -> String {
        let steps = self.steps.iter().map(|step| {
            let mark = match step.state {
                StepState::Ok => "ok",
                StepState::Failed => "failed",
                StepState::NotChecked => "not checked",
            };
            match step.detail.as_deref().filter(|d| !d.is_empty()) {
                Some(detail) => format!("- {}: {mark} ({detail})", step.label),
                None => format!("- {}: {mark}", step.label),
            }
        });
        std::iter::once(self.headline.clone())
            .chain(steps)
            .chain(std::iter::once(format!("Try: {}", self.next)))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
