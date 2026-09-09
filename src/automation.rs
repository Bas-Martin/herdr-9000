use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_AUTOMATION_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_RUN_ID: AtomicU64 = AtomicU64::new(1);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AutomationRunStatus {
    Queued,
    Provisioning,
    Launching,
    Completed,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AutomationRun {
    pub(crate) run_id: String,
    pub(crate) scheduled_slot: u64,
    pub(crate) status: AutomationRunStatus,
    pub(crate) started_at: u64,
    pub(crate) finished_at: Option<u64>,
    pub(crate) task_id: Option<String>,
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Automation {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) project_id: String,
    pub(crate) cron: String,
    pub(crate) prompt: String,
    pub(crate) provider: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) workspace_mode: crate::task::TaskLocationMode,
    pub(crate) enabled: bool,
    pub(crate) paused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) last_scheduled_slot: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) runs: Vec<AutomationRun>,
    pub(crate) created_at: u64,
    pub(crate) updated_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) struct AutomationRegistry {
    #[serde(default = "default_registry_version")]
    pub(crate) version: u32,
    #[serde(default)]
    pub(crate) automations: Vec<Automation>,
}

fn default_registry_version() -> u32 {
    1
}

impl Default for AutomationRegistry {
    fn default() -> Self {
        Self {
            version: default_registry_version(),
            automations: Vec::new(),
        }
    }
}

impl AutomationRegistry {
    pub(crate) fn find(&self, id: &str) -> Option<&Automation> {
        self.automations
            .iter()
            .find(|automation| automation.id == id)
    }

    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Automation> {
        self.automations
            .iter_mut()
            .find(|automation| automation.id == id)
    }

    pub(crate) fn insert(&mut self, automation: Automation) {
        self.automations.push(automation);
    }

    pub(crate) fn remove(&mut self, id: &str) -> Option<Automation> {
        let index = self
            .automations
            .iter()
            .position(|automation| automation.id == id)?;
        Some(self.automations.remove(index))
    }
}

impl Automation {
    pub(crate) fn new(
        name: String,
        project_id: String,
        cron: String,
        prompt: String,
        provider: Option<String>,
        model: Option<String>,
        workspace_mode: crate::task::TaskLocationMode,
    ) -> Self {
        let now = current_unix_ms();
        Self {
            id: format!("a{}", NEXT_AUTOMATION_ID.fetch_add(1, Ordering::Relaxed)),
            name,
            project_id,
            cron,
            prompt,
            provider,
            model,
            workspace_mode,
            enabled: true,
            paused: false,
            last_scheduled_slot: None,
            runs: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub(crate) fn append_run(&mut self, run: AutomationRun) {
        self.runs.push(run);
        const MAX_RUNS: usize = 50;
        if self.runs.len() > MAX_RUNS {
            let remove = self.runs.len() - MAX_RUNS;
            self.runs.drain(..remove);
        }
    }
}

impl AutomationRun {
    pub(crate) fn new(scheduled_slot: u64, status: AutomationRunStatus) -> Self {
        Self {
            run_id: format!("ar{}", NEXT_RUN_ID.fetch_add(1, Ordering::Relaxed)),
            scheduled_slot,
            status,
            started_at: current_unix_ms(),
            finished_at: None,
            task_id: None,
            error: None,
        }
    }
}

pub(crate) fn reserve_automation_ids(registry: &AutomationRegistry) {
    let next_id = registry
        .automations
        .iter()
        .filter_map(|automation| automation.id.strip_prefix('a'))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    NEXT_AUTOMATION_ID.fetch_max(next_id, Ordering::Relaxed);

    let next_run_id = registry
        .automations
        .iter()
        .flat_map(|automation| automation.runs.iter())
        .filter_map(|run| run.run_id.strip_prefix("ar"))
        .filter_map(|value| value.parse::<u64>().ok())
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    NEXT_RUN_ID.fetch_max(next_run_id, Ordering::Relaxed);
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

pub(crate) fn current_cron_slot(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() / 60)
        .unwrap_or(0)
}

pub(crate) fn cron_matches(expression: &str, slot: u64) -> Result<bool, String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err("cron must contain five fields: minute hour day month weekday".to_owned());
    }
    let minute = slot.saturating_mul(60);
    let total_days = minute / 86_400;
    let second_of_day = minute % 86_400;
    let hour = second_of_day / 3_600;
    let minute_of_hour = (second_of_day % 3_600) / 60;
    let (_, month, day) = civil_from_days(total_days as i64);
    let weekday = (total_days + 4) % 7;

    let minute_match = cron_field_matches(fields[0], minute_of_hour, 0, 59)?;
    let hour_match = cron_field_matches(fields[1], hour, 0, 23)?;
    let day_match = cron_field_matches(fields[2], u64::from(day), 1, 31)?;
    let month_match = cron_field_matches(fields[3], u64::from(month), 1, 12)?;
    let weekday_match = cron_field_matches(fields[4], weekday, 0, 7)?
        || (weekday == 0 && cron_field_matches(fields[4], 7, 0, 7)?);
    let day_restricted = fields[2] != "*";
    let weekday_restricted = fields[4] != "*";
    let day_matches = if day_restricted && weekday_restricted {
        day_match || weekday_match
    } else {
        day_match && weekday_match
    };
    Ok(minute_match && hour_match && day_matches && month_match)
}

fn cron_field_matches(field: &str, value: u64, minimum: u64, maximum: u64) -> Result<bool, String> {
    if field.is_empty() {
        return Err("cron fields must not be empty".to_owned());
    }
    for part in field.split(',') {
        if part.is_empty() {
            return Err("cron list contains an empty value".to_owned());
        }
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step = step
                    .parse::<u64>()
                    .map_err(|_| format!("invalid cron step: {step}"))?;
                if step == 0 {
                    return Err("cron step must be greater than zero".to_owned());
                }
                (range, step)
            }
            None => (part, 1),
        };
        let (start, end) = if range == "*" {
            (minimum, maximum)
        } else if let Some((start, end)) = range.split_once('-') {
            (
                parse_cron_value(start, minimum, maximum)?,
                parse_cron_value(end, minimum, maximum)?,
            )
        } else {
            let value = parse_cron_value(range, minimum, maximum)?;
            (value, value)
        };
        if start > end {
            return Err(format!("cron range must be ascending: {range}"));
        }
        if value >= start && value <= end && (value - start).is_multiple_of(step) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_cron_value(value: &str, minimum: u64, maximum: u64) -> Result<u64, String> {
    let parsed = value
        .parse::<u64>()
        .map_err(|_| format!("invalid cron value: {value}"))?;
    if parsed < minimum || parsed > maximum {
        return Err(format!(
            "cron value {parsed} is outside {minimum}..{maximum}"
        ));
    }
    Ok(parsed)
}

// Howard Hinnant's civil-from-days conversion, with Unix epoch days as input.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let adjusted = days + 719_468;
    let era = if adjusted >= 0 {
        adjusted / 146_097
    } else {
        (adjusted - 146_096) / 146_097
    };
    let day_of_era = adjusted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_part + 2) / 5 + 1;
    let month = month_part + if month_part < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}
