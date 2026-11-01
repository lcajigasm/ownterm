//! Local cost router for the `auto` model id.
//!
//! Warp's custom model routers are YAML maps sent to Warp's servers. This one
//! stays on the machine: a checked-in catalog, a fixed classifier, and the
//! cheapest available model that meets the task. API keys are never written.

use std::time::Duration;

use anyhow::{Context, anyhow, bail};
use url::Url;

use crate::{
    ENV_API_KEY, ENV_MODEL, ENV_SESSION_MODEL, ProviderInputs, find_preset, read_provider_file,
    resolve,
};

pub const AUTO_MODEL: &str = "auto";
const CATALOG_JSON: &str = include_str!("../catalog.json");
const ATTEMPT_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskClass {
    Command,
    Chat,
    Agent,
    LongContext,
}

impl TaskClass {
    pub fn label(self) -> &'static str {
        match self {
            Self::Command => "a command",
            Self::Chat => "a short chat",
            Self::Agent => "an agent task",
            Self::LongContext => "a long context",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptFailure {
    Error,
    RateLimit,
    Timeout,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogModel {
    pub provider: String,
    pub id: String,
    pub input_micros: i64,
    pub output_micros: i64,
    pub price_exact: bool,
    pub context_tokens: u32,
    pub tools: bool,
    pub vision: bool,
    pub reasoning: bool,
    pub code: bool,
    pub quality: u8,
    pub api_key_env: Option<String>,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Access {
    pub provider: String,
    pub key_present: bool,
    pub reachable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub provider: String,
    pub model: String,
    pub input_micros: i64,
    pub output_micros: i64,
    pub price_exact: bool,
    pub quality: u8,
    pub context_tokens: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    pub provider: String,
    pub model: String,
    pub endpoint: Url,
    pub api_key: Option<String>,
    pub reason: String,
}

pub enum ModelChoice {
    Explicit(crate::ResolvedEndpoint),
    Routed(Vec<Attempt>),
}

pub fn priced_on() -> &'static str {
    catalog().priced_on.as_str()
}

pub fn catalog_models() -> &'static [CatalogModel] {
    &catalog().models
}

pub fn classify(prompt: &str) -> TaskClass {
    let trimmed = prompt.trim();
    let lower = trimmed.to_ascii_lowercase();
    if is_agent(&lower) {
        return TaskClass::Agent;
    }
    if is_long(trimmed) {
        return TaskClass::LongContext;
    }
    if is_command(trimmed, &lower) {
        return TaskClass::Command;
    }
    TaskClass::Chat
}

pub fn classify_failure(status: Option<u16>, message: &str) -> AttemptFailure {
    if status == Some(429) {
        return AttemptFailure::RateLimit;
    }
    if status == Some(408) || status == Some(504) {
        return AttemptFailure::Timeout;
    }
    let lower = message.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("timeout") {
        return AttemptFailure::Timeout;
    }
    AttemptFailure::Error
}

pub fn should_fallback(failure: AttemptFailure) -> bool {
    match failure {
        AttemptFailure::Error | AttemptFailure::RateLimit | AttemptFailure::Timeout => true,
    }
}

pub fn rank(
    class: TaskClass,
    prompt_chars: usize,
    models: &[CatalogModel],
    access: &[Access],
    preferred_provider: Option<&str>,
) -> Result<Vec<Candidate>, String> {
    let need = requirements(class, prompt_chars);
    let mut fits: Vec<&CatalogModel> = models
        .iter()
        .filter(|model| model_available(model, access))
        .filter(|model| model_fits(model, &need))
        .collect();
    fits.sort_by(|left, right| {
        cost(left)
            .cmp(&cost(right))
            .then(right.quality.cmp(&left.quality))
            .then(prefer(left, preferred_provider).cmp(&prefer(right, preferred_provider)))
            .then(left.provider.cmp(&right.provider))
            .then(left.id.cmp(&right.id))
    });
    if fits.is_empty() {
        return Err(
            "Auto found no available model for this task. Start a local server or set a provider key."
                .to_owned(),
        );
    }
    Ok(fits.iter().copied().map(to_candidate).collect())
}

pub fn next_candidate<'a>(ranked: &'a [Candidate], failed: &Candidate) -> Option<&'a Candidate> {
    let mut seen = false;
    for candidate in ranked {
        if seen {
            return Some(candidate);
        }
        if candidate.provider == failed.provider && candidate.model == failed.model {
            seen = true;
        }
    }
    None
}

pub fn reason_line(
    class: TaskClass,
    candidate: &Candidate,
    fell_back_from: Option<&str>,
) -> String {
    let price = price_phrase(candidate);
    match fell_back_from {
        Some(previous) => format!(
            "Auto fell back to {}/{} after {previous} failed — {price}, quality {}, context {}",
            candidate.provider, candidate.model, candidate.quality, candidate.context_tokens
        ),
        None => format!(
            "Auto: {}/{} for {} — {price}, quality {}, context {}",
            candidate.provider,
            candidate.model,
            class.label(),
            candidate.quality,
            candidate.context_tokens
        ),
    }
}

pub fn normalize_pin(text: &str) -> Result<Option<(String, String)>, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case(AUTO_MODEL) {
        return Ok(None);
    }
    let models = catalog_models();
    if let Some((provider, model)) = text.split_once('/')
        && find_preset(provider).is_some()
    {
        let provider = find_preset(provider)
            .map(|preset| preset.id)
            .unwrap_or(provider);
        if models
            .iter()
            .any(|entry| entry.provider == provider && entry.id == model)
        {
            return Ok(Some((provider.to_owned(), model.to_owned())));
        }
        return Err(format!("Unknown model {provider}/{model}."));
    }
    let matches: Vec<_> = models.iter().filter(|entry| entry.id == text).collect();
    match matches.as_slice() {
        [only] => Ok(Some((only.provider.clone(), only.id.clone()))),
        [] => Err(format!(
            "Unknown model {text}. Use provider/model, for example openai/gpt-4o-mini."
        )),
        _ => Err(format!(
            "{text} is offered by more than one provider. Use provider/model."
        )),
    }
}

pub fn settings_line(provider: &str, model: &str, session_pin: &str) -> String {
    if let Ok(Some((pin_provider, pin_model))) = normalize_pin(session_pin) {
        return format!("Session pin: {pin_provider}/{pin_model}. Auto is not choosing.");
    }
    if !model.trim().eq_ignore_ascii_case(AUTO_MODEL) {
        let model = model.trim();
        if model.is_empty() {
            return "Set the model to Auto, or name one.".to_owned();
        }
        return format!("Default model is {model}. Set the model to Auto to route by cost.");
    }
    let access = access_from_environment();
    match rank(
        TaskClass::Chat,
        "hello".len(),
        catalog_models(),
        &access,
        Some(provider),
    ) {
        Ok(ranked) => {
            let line = reason_line(TaskClass::Chat, &ranked[0], None);
            match read_last_choice() {
                Some(last) => format!("{line} Last request: {last}"),
                None => line,
            }
        }
        Err(message) => message,
    }
}

pub fn preview(prompt: &str) -> Result<Vec<String>, String> {
    let class = classify(prompt);
    let preferred = read_provider_file()
        .ok()
        .flatten()
        .and_then(|file| file.provider);
    let ranked = rank(
        class,
        prompt.chars().count(),
        catalog_models(),
        &access_from_environment(),
        preferred.as_deref(),
    )?;
    let mut lines = Vec::with_capacity(ranked.len());
    for (index, candidate) in ranked.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .map(|earlier| format!("{}/{}", ranked[earlier].provider, ranked[earlier].model));
        lines.push(reason_line(class, candidate, previous.as_deref()));
    }
    Ok(lines)
}

pub fn choose_model(prompt: &str) -> anyhow::Result<Option<ModelChoice>> {
    if let Some((provider, model)) = session_pin()? {
        let attempt = pinned_attempt(&provider, &model)?;
        return Ok(Some(ModelChoice::Routed(vec![attempt])));
    }
    let Some(configured) = configured_model()? else {
        return Ok(None);
    };
    let Configured {
        provider,
        model,
        base_url,
        api_key_env,
    } = configured;
    if !model.eq_ignore_ascii_case(AUTO_MODEL) {
        let endpoint = resolve(&ProviderInputs {
            provider: Some(provider),
            base_url,
            model: Some(model),
            api_key: configured_key(api_key_env.as_deref()),
            api_key_env,
        })?;
        return Ok(Some(ModelChoice::Explicit(endpoint)));
    }
    let class = classify(prompt);
    let access = access_from_environment();
    let ranked = rank(
        class,
        prompt.chars().count(),
        catalog_models(),
        &access,
        Some(&provider),
    )
    .map_err(|message| anyhow!(message))?;
    let mut attempts = Vec::with_capacity(ranked.len());
    for (index, candidate) in ranked.iter().enumerate() {
        let previous = index
            .checked_sub(1)
            .map(|earlier| format!("{}/{}", ranked[earlier].provider, ranked[earlier].model));
        attempts.push(attempt_for(
            candidate,
            reason_line(class, candidate, previous.as_deref()),
        )?);
    }
    Ok(Some(ModelChoice::Routed(attempts)))
}

pub fn write_session_pin(text: &str) -> Result<(), String> {
    let pin = normalize_pin(text)?;
    let path = session_model_path().map_err(|err| err.to_string())?;
    match pin {
        None => {
            if path.is_file() {
                std::fs::remove_file(&path).map_err(|err| err.to_string())?;
            }
            Ok(())
        }
        Some((provider, model)) => write_private_line(&path, &format!("{provider}/{model}\n"))
            .map_err(|err| err.to_string()),
    }
}

pub fn read_session_pin_text() -> String {
    session_pin()
        .ok()
        .flatten()
        .map(|(provider, model)| format!("{provider}/{model}"))
        .unwrap_or_default()
}

pub fn record_last_choice(line: &str) {
    let Ok(path) = last_choice_path() else {
        return;
    };
    let _ = write_private_line(&path, &format!("{}\n", line.trim()));
}

pub fn loopback_port_open(base_url: &str) -> bool {
    let Ok(url) = Url::parse(base_url) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if !loopback {
        return false;
    }
    let port = url.port_or_known_default().unwrap_or(80);
    let address = if host.eq_ignore_ascii_case("localhost") {
        format!("127.0.0.1:{port}")
    } else {
        format!("{host}:{port}")
    };
    std::net::TcpStream::connect_timeout(
        &address
            .parse()
            .unwrap_or(std::net::SocketAddr::from(([127, 0, 0, 1], port))),
        ATTEMPT_TIMEOUT,
    )
    .is_ok()
}

fn is_agent(lower: &str) -> bool {
    const MARKERS: &[&str] = &[
        "step by step",
        "and then",
        "after that",
        "implement",
        "refactor",
        "fix the",
        "write a ",
        "edit ",
        "create a file",
        "run the tests",
    ];
    MARKERS.iter().any(|marker| lower.contains(marker)) || has_numbered_steps(lower)
}

fn has_numbered_steps(lower: &str) -> bool {
    lower.contains("\n1.") && lower.contains("\n2.")
        || lower.contains("\n1)") && lower.contains("\n2)")
}

fn is_long(prompt: &str) -> bool {
    if prompt.chars().count() > 6000 || prompt.lines().count() > 80 {
        return true;
    }
    let fence = prompt.matches("```").count();
    if fence >= 2 {
        let inside = prompt.split("```").nth(1).unwrap_or("");
        if inside.lines().count() > 40 {
            return true;
        }
    }
    prompt.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("diff --git") || trimmed.starts_with("@@")
    })
}

fn is_command(prompt: &str, lower: &str) -> bool {
    if prompt.lines().count() != 1 || prompt.chars().count() > 120 {
        return false;
    }
    let Some(first) = lower.split_whitespace().next() else {
        return false;
    };
    const SHELL: &[&str] = &[
        "ls", "git", "cd", "cargo", "npm", "kubectl", "curl", "grep", "find", "sudo", "cat",
        "echo", "make", "docker", "python", "rg", "pwd", "mkdir",
    ];
    if SHELL.contains(&first) {
        return true;
    }
    (lower.starts_with("how do i ") || lower.starts_with("how to "))
        && prompt.chars().count() <= 64
        && !lower.contains("implement")
}

struct Need {
    min_quality: u8,
    min_context: u32,
    tools: bool,
    code: bool,
}

fn requirements(class: TaskClass, prompt_chars: usize) -> Need {
    let estimated = u32::try_from(prompt_chars / 4 + 1024).unwrap_or(u32::MAX);
    let (floor, min_quality, tools, code) = match class {
        TaskClass::Command => (4_096, 2, false, false),
        TaskClass::Chat => (8_192, 2, false, false),
        TaskClass::Agent => (16_384, 3, true, false),
        TaskClass::LongContext => (32_768, 3, false, true),
    };
    Need {
        min_quality,
        min_context: floor.max(estimated),
        tools,
        code,
    }
}

fn model_fits(model: &CatalogModel, need: &Need) -> bool {
    model.quality >= need.min_quality
        && model.context_tokens >= need.min_context
        && (!need.tools || model.tools)
        && (!need.code || model.code)
}

fn model_available(model: &CatalogModel, access: &[Access]) -> bool {
    let Some(access) = access.iter().find(|item| item.provider == model.provider) else {
        return false;
    };
    if model.api_key_env.is_some() {
        access.key_present
    } else {
        access.reachable
    }
}

fn cost(model: &CatalogModel) -> i64 {
    model.input_micros.saturating_add(model.output_micros)
}

fn prefer(model: &CatalogModel, preferred: Option<&str>) -> u8 {
    match preferred {
        Some(provider) if model.provider.eq_ignore_ascii_case(provider) => 0,
        _ => 1,
    }
}

fn to_candidate(model: &CatalogModel) -> Candidate {
    Candidate {
        provider: model.provider.clone(),
        model: model.id.clone(),
        input_micros: model.input_micros,
        output_micros: model.output_micros,
        price_exact: model.price_exact,
        quality: model.quality,
        context_tokens: model.context_tokens,
    }
}

fn price_phrase(candidate: &Candidate) -> String {
    if candidate.input_micros == 0 && candidate.output_micros == 0 {
        return "local, $0".to_owned();
    }
    let input = format_usd(candidate.input_micros);
    let output = format_usd(candidate.output_micros);
    if candidate.price_exact {
        format!("{input} in / {output} out per 1M")
    } else {
        format!("~{input} in / ~{output} out per 1M, approximate")
    }
}

fn format_usd(micros: i64) -> String {
    let whole = micros / 1_000_000;
    let frac = (micros.abs() % 1_000_000) as u32;
    if frac == 0 {
        return format!("${whole}");
    }
    let mut fraction = format!("{frac:06}");
    while fraction.ends_with('0') {
        fraction.pop();
    }
    format!("${whole}.{fraction}")
}

fn pinned_attempt(provider: &str, model: &str) -> anyhow::Result<Attempt> {
    let candidate = catalog_models()
        .iter()
        .find(|entry| entry.provider == provider && entry.id == model)
        .context("pinned model left the catalog")?;
    attempt_for(
        &to_candidate(candidate),
        format!("Session pin: {provider}/{model}. Auto is not choosing."),
    )
}

fn attempt_for(candidate: &Candidate, reason: String) -> anyhow::Result<Attempt> {
    let preset = find_preset(&candidate.provider)
        .with_context(|| format!("unknown provider {}", candidate.provider))?;
    let base_url = preset
        .default_base_url
        .context("auto catalog provider has no base URL")?;
    let api_key_env = catalog_models()
        .iter()
        .find(|entry| entry.provider == candidate.provider && entry.id == candidate.model)
        .and_then(|entry| entry.api_key_env.clone());
    let api_key = key_for(candidate.provider.as_str(), api_key_env.as_deref());
    let endpoint = resolve(&ProviderInputs {
        provider: Some(candidate.provider.clone()),
        base_url: Some(base_url.to_owned()),
        model: Some(candidate.model.clone()),
        api_key: api_key.clone(),
        api_key_env,
    })?;
    Ok(Attempt {
        provider: candidate.provider.clone(),
        model: candidate.model.clone(),
        endpoint: endpoint.endpoint,
        api_key,
        reason,
    })
}

fn key_for(provider: &str, catalog_env: Option<&str>) -> Option<String> {
    if let Some(name) = catalog_env
        && let Some(value) = nonempty_env(name)
    {
        return Some(value);
    }
    let file = read_provider_file().ok().flatten()?;
    let file_provider = file.provider.as_deref()?;
    if !file_provider.eq_ignore_ascii_case(provider) {
        return None;
    }
    let name = file.api_key_env.as_deref().unwrap_or(ENV_API_KEY);
    nonempty_env(name)
}

fn configured_key(api_key_env: Option<&str>) -> Option<String> {
    nonempty_env(api_key_env.unwrap_or(ENV_API_KEY))
}

struct Configured {
    provider: String,
    model: String,
    base_url: Option<String>,
    api_key_env: Option<String>,
}

fn configured_model() -> anyhow::Result<Option<Configured>> {
    let file = read_provider_file()?;
    let provider = crate::env_or(
        crate::ENV_PROVIDER,
        file.as_ref().and_then(|file| file.provider.clone()),
    );
    let model = crate::env_or(ENV_MODEL, file.as_ref().and_then(|file| file.model.clone()));
    let base_url = crate::env_or(
        crate::ENV_BASE_URL,
        file.as_ref().and_then(|file| file.base_url.clone()),
    );
    let api_key_env = file.and_then(|file| file.api_key_env);
    match (provider, model) {
        (None, None) if base_url.is_none() => Ok(None),
        (provider, model) => Ok(Some(Configured {
            provider: provider.unwrap_or_else(|| "compatible".to_owned()),
            model: model.unwrap_or_default(),
            base_url,
            api_key_env,
        })),
    }
}

fn session_pin() -> anyhow::Result<Option<(String, String)>> {
    if let Some(from_env) = nonempty_env(ENV_SESSION_MODEL) {
        return normalize_pin(&from_env).map_err(|message| anyhow!(message));
    }
    let path = session_model_path()?;
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    normalize_pin(text.trim()).map_err(|message| anyhow!(message))
}

pub fn access_from_environment() -> Vec<Access> {
    catalog_models()
        .iter()
        .map(|model| model.provider.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(|provider| {
            let preset = find_preset(provider);
            let reachable = preset
                .and_then(|preset| preset.default_base_url)
                .is_some_and(loopback_port_open);
            let key_present = catalog_models().iter().any(|model| {
                model.provider == provider
                    && model
                        .api_key_env
                        .as_deref()
                        .is_some_and(|name| nonempty_env(name).is_some())
            }) || key_for(provider, None).is_some();
            Access {
                provider: provider.to_owned(),
                key_present,
                reachable,
            }
        })
        .collect()
}

fn read_last_choice() -> Option<String> {
    let path = last_choice_path().ok()?;
    let text = std::fs::read_to_string(path).ok()?;
    let line = text.trim();
    (!line.is_empty()).then(|| line.to_owned())
}

fn session_model_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::dirs_home()
        .context("HOME is not set")?
        .join(".ownterm")
        .join("session-model"))
}

fn last_choice_path() -> anyhow::Result<std::path::PathBuf> {
    Ok(crate::dirs_home()
        .context("HOME is not set")?
        .join(".ownterm")
        .join("last-auto.txt"))
}

fn write_private_line(path: &std::path::Path, text: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

fn nonempty_env(name: &str) -> Option<String> {
    crate::nonempty_owned(std::env::var(name).ok())
}

struct CatalogFile {
    priced_on: String,
    models: Vec<CatalogModel>,
}

fn catalog() -> &'static CatalogFile {
    static CATALOG: std::sync::OnceLock<CatalogFile> = std::sync::OnceLock::new();
    CATALOG.get_or_init(|| parse_catalog(CATALOG_JSON).expect("ownterm catalog.json is valid"))
}

fn parse_catalog(text: &str) -> anyhow::Result<CatalogFile> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let priced_on = value
        .get("priced_on")
        .and_then(|item| item.as_str())
        .context("catalog priced_on")?
        .to_owned();
    let rows = value
        .get("models")
        .and_then(|item| item.as_array())
        .context("catalog models")?;
    let mut models = Vec::new();
    for row in rows {
        let provider = required_str(row, "provider")?;
        let id = required_str(row, "id")?;
        let quality = row
            .get("quality")
            .and_then(|item| item.as_u64())
            .context("quality")?;
        if !(1..=5).contains(&quality) {
            bail!("quality for {provider}/{id} must be 1..=5");
        }
        models.push(CatalogModel {
            provider,
            id,
            input_micros: parse_usd(&required_str(row, "input_usd_per_million")?)?,
            output_micros: parse_usd(&required_str(row, "output_usd_per_million")?)?,
            price_exact: row
                .get("price_exact")
                .and_then(|item| item.as_bool())
                .context("price_exact")?,
            context_tokens: row
                .get("context_tokens")
                .and_then(|item| item.as_u64())
                .and_then(|value| u32::try_from(value).ok())
                .context("context_tokens")?,
            tools: flag(row, "tools")?,
            vision: flag(row, "vision")?,
            reasoning: flag(row, "reasoning")?,
            code: flag(row, "code")?,
            quality: u8::try_from(quality)?,
            api_key_env: row
                .get("api_key_env")
                .and_then(|item| item.as_str())
                .map(str::to_owned),
            source: required_str(row, "source")?,
        });
    }
    if models.is_empty() {
        bail!("catalog has no models");
    }
    Ok(CatalogFile { priced_on, models })
}

fn required_str(row: &serde_json::Value, key: &str) -> anyhow::Result<String> {
    row.get(key)
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("catalog field {key}"))
}

fn flag(row: &serde_json::Value, key: &str) -> anyhow::Result<bool> {
    row.get(key)
        .and_then(|item| item.as_bool())
        .with_context(|| format!("catalog field {key}"))
}

fn parse_usd(text: &str) -> anyhow::Result<i64> {
    let text = text.trim();
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    if whole.is_empty() || !whole.chars().all(|ch| ch.is_ascii_digit()) {
        bail!("bad USD amount {text}");
    }
    if fraction.len() > 6 || !fraction.chars().all(|ch| ch.is_ascii_digit()) {
        bail!("bad USD amount {text}");
    }
    let whole: i64 = whole.parse()?;
    let mut padded = fraction.to_owned();
    while padded.len() < 6 {
        padded.push('0');
    }
    let fraction: i64 = if padded.is_empty() {
        0
    } else {
        padded.parse()?
    };
    Ok(whole * 1_000_000 + fraction)
}

#[cfg(test)]
#[path = "auto_tests.rs"]
mod tests;
