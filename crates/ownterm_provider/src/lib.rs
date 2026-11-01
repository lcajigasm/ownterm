//! Resolves an OpenAI-compatible chat endpoint for OwnTerm.
//!
//! API keys are read from the environment variable named by the provider file
//! (`OWNTERM_API_KEY` when the file does not name one). The file may name the
//! provider, base URL, model, and that variable, and is rejected if it contains a secret.

mod form;

pub use form::{ProviderForm, load_provider_form, model_choices, save_provider_form};

use anyhow::{Context, anyhow, bail};
use url::Url;

pub const ENV_PROVIDER: &str = "OWNTERM_PROVIDER";
pub const ENV_BASE_URL: &str = "OWNTERM_BASE_URL";
pub const ENV_MODEL: &str = "OWNTERM_MODEL";
pub const ENV_API_KEY: &str = "OWNTERM_API_KEY";
pub const ENV_PROVIDER_FILE: &str = "OWNTERM_PROVIDER_FILE";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProviderPreset {
    pub id: &'static str,
    pub display_name: &'static str,
    pub default_base_url: Option<&'static str>,
    pub requires_api_key: bool,
}

const PRESETS: &[ProviderPreset] = &[
    ProviderPreset {
        id: "ollama",
        display_name: "Ollama",
        default_base_url: Some("http://127.0.0.1:11434/v1"),
        requires_api_key: false,
    },
    ProviderPreset {
        id: "lmstudio",
        display_name: "LM Studio",
        default_base_url: Some("http://127.0.0.1:1234/v1"),
        requires_api_key: false,
    },
    ProviderPreset {
        id: "vllm",
        display_name: "vLLM",
        default_base_url: Some("http://127.0.0.1:8000/v1"),
        requires_api_key: false,
    },
    ProviderPreset {
        id: "localai",
        display_name: "LocalAI",
        default_base_url: Some("http://127.0.0.1:8080/v1"),
        requires_api_key: false,
    },
    ProviderPreset {
        id: "openai",
        display_name: "OpenAI",
        default_base_url: Some("https://api.openai.com/v1"),
        requires_api_key: true,
    },
    ProviderPreset {
        id: "xai",
        display_name: "xAI",
        default_base_url: Some("https://api.x.ai/v1"),
        requires_api_key: true,
    },
    ProviderPreset {
        id: "openrouter",
        display_name: "OpenRouter",
        default_base_url: Some("https://openrouter.ai/api/v1"),
        requires_api_key: true,
    },
    ProviderPreset {
        id: "groq",
        display_name: "Groq",
        default_base_url: Some("https://api.groq.com/openai/v1"),
        requires_api_key: true,
    },
    ProviderPreset {
        id: "compatible",
        display_name: "OpenAI-compatible",
        default_base_url: None,
        requires_api_key: false,
    },
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEndpoint {
    pub provider_id: String,
    pub endpoint: Url,
    pub model: String,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProviderInputs {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    /// Environment variable that holds the key. The file stores this name, not the key.
    pub api_key_env: Option<String>,
}

pub fn presets() -> &'static [ProviderPreset] {
    PRESETS
}

pub fn find_preset(id: &str) -> Option<&'static ProviderPreset> {
    let id = id.trim();
    PRESETS
        .iter()
        .find(|preset| preset.id.eq_ignore_ascii_case(id))
}

/// True when process environment or a provider file names a direct endpoint.
///
/// Warp credit checks use this so a configured local or BYO model is not blocked
/// by a subscription quota.
pub fn direct_inference_configured() -> bool {
    if env_nonempty(ENV_PROVIDER) || env_nonempty(ENV_BASE_URL) || env_nonempty(ENV_MODEL) {
        return true;
    }
    match read_provider_file() {
        Ok(Some(file)) => {
            file.provider.is_some() || file.base_url.is_some() || file.model.is_some()
        }
        Ok(None) => false,
        // A present but unreadable file is still an explicit provider choice.
        // The request path surfaces the parse error instead of a Warp credit gate.
        Err(_) => true,
    }
}

pub fn resolve_from_process_env() -> anyhow::Result<Option<ResolvedEndpoint>> {
    let file = read_provider_file()?;
    let api_key_env = file
        .as_ref()
        .and_then(|file| file.api_key_env.clone())
        .unwrap_or_else(|| ENV_API_KEY.to_owned());
    let inputs = ProviderInputs {
        provider: env_or(ENV_PROVIDER, file.as_ref().and_then(|f| f.provider.clone())),
        base_url: env_or(ENV_BASE_URL, file.as_ref().and_then(|f| f.base_url.clone())),
        model: env_or(ENV_MODEL, file.as_ref().and_then(|f| f.model.clone())),
        api_key: nonempty_owned(std::env::var(&api_key_env).ok()),
        api_key_env: Some(api_key_env),
    };
    if inputs.provider.is_none() && inputs.base_url.is_none() && inputs.model.is_none() {
        return Ok(None);
    }
    resolve(&inputs).map(Some)
}

pub fn resolve(inputs: &ProviderInputs) -> anyhow::Result<ResolvedEndpoint> {
    let preset = match inputs
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(id) => {
            Some(find_preset(id).with_context(|| format!("unknown OWNTERM_PROVIDER '{id}'"))?)
        }
        None => None,
    };
    let base_url = nonempty(inputs.base_url.as_deref())
        .map(str::to_owned)
        .or_else(|| preset.and_then(|preset| preset.default_base_url.map(str::to_owned)))
        .context("set OWNTERM_BASE_URL or OWNTERM_PROVIDER")?;
    let model = nonempty(inputs.model.as_deref())
        .map(str::to_owned)
        .context("OWNTERM_MODEL is required")?;
    let api_key = nonempty(inputs.api_key.as_deref()).map(str::to_owned);
    if preset.is_some_and(|preset| preset.requires_api_key) && api_key.is_none() {
        let id = preset.map(|preset| preset.id).unwrap_or("provider");
        let env_name = inputs
            .api_key_env
            .as_deref()
            .filter(|name| !name.is_empty())
            .unwrap_or(ENV_API_KEY);
        bail!("set {env_name} in the environment for the {id} provider; the key is not stored");
    }
    Ok(ResolvedEndpoint {
        provider_id: preset
            .map(|preset| preset.id.to_owned())
            .unwrap_or_else(|| "compatible".to_owned()),
        endpoint: chat_completions_url(&base_url)?,
        model,
        api_key,
    })
}

pub fn chat_completions_url(base_url: &str) -> anyhow::Result<Url> {
    let mut url = Url::parse(base_url).context("invalid OpenAI-compatible base URL")?;
    let is_loopback = url.host_str().is_some_and(|host| {
        host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() != "https" && !(url.scheme() == "http" && is_loopback) {
        return Err(anyhow!(
            "OwnTerm model endpoints must use HTTPS; HTTP is allowed only for loopback hosts"
        ));
    }
    if !url
        .path()
        .trim_end_matches('/')
        .ends_with("/chat/completions")
    {
        let path = format!("{}/chat/completions", url.path().trim_end_matches('/'));
        url.set_path(&path);
    }
    Ok(url)
}

/// OpenAI `/models` URL for a chat base URL. Does not contact the network.
pub fn models_list_url(base_url: &str) -> anyhow::Result<Url> {
    let mut url = chat_completions_url(base_url)?;
    let path = url.path().trim_end_matches('/').to_owned();
    let prefix = path
        .strip_suffix("/chat/completions")
        .unwrap_or(path.as_str())
        .trim_end_matches('/');
    url.set_path(&format!("{prefix}/models"));
    Ok(url)
}

/// Catalog URL for a preset, using `base_url` when it is set.
pub fn models_list_url_for_provider(provider: &str, base_url: Option<&str>) -> anyhow::Result<Url> {
    let preset = find_preset(provider).with_context(|| format!("unknown provider '{provider}'"))?;
    let base = nonempty(base_url)
        .map(str::to_owned)
        .or_else(|| preset.default_base_url.map(str::to_owned))
        .context("set a base URL for this provider")?;
    models_list_url(&base)
}

/// Static model ids a user can pass to `OWNTERM_MODEL`. Not a live catalog.
pub fn suggested_models(provider_id: &str) -> &'static [&'static str] {
    match find_preset(provider_id).map(|preset| preset.id) {
        Some("ollama") => &["llama3.3", "qwen2.5-coder:7b"],
        Some("lmstudio") => &["qwen2.5-coder-7b-instruct"],
        Some("vllm") => &["Qwen/Qwen2.5-Coder-7B-Instruct"],
        Some("localai") => &["llama3.3"],
        Some("openai") => &["gpt-4o", "gpt-4o-mini"],
        Some("xai") => &["grok-3", "grok-3-mini"],
        Some("openrouter") => &["openai/gpt-4o-mini"],
        Some("groq") => &["llama-3.3-70b-versatile"],
        _ => &[],
    }
}

/// Model ids from an OpenAI `/models` document (`{"data":[{"id":"..."}]}`).
pub fn parse_model_ids(body: &str) -> anyhow::Result<Vec<String>> {
    let value: serde_json::Value =
        serde_json::from_str(body).context("model catalog is not JSON")?;
    let data = value
        .get("data")
        .and_then(|data| data.as_array())
        .context("model catalog is missing data")?;
    let mut ids = Vec::new();
    for item in data {
        let Some(id) = item.get("id").and_then(|id| id.as_str()) else {
            continue;
        };
        let id = id.trim();
        if !id.is_empty() {
            ids.push(id.to_owned());
        }
    }
    Ok(ids)
}

/// `~/.ownterm/provider.env`, or `OWNTERM_PROVIDER_FILE` when that variable is set.
pub fn default_provider_file_path() -> anyhow::Result<std::path::PathBuf> {
    if let Some(path) = nonempty_owned(std::env::var(ENV_PROVIDER_FILE).ok()) {
        return Ok(std::path::PathBuf::from(path));
    }
    let home = dirs_home().context("HOME is not set")?;
    Ok(home.join(".ownterm").join("provider.env"))
}

pub fn format_provider_file(
    provider: &str,
    model: &str,
    base_url: Option<&str>,
) -> anyhow::Result<String> {
    let preset = find_preset(provider).with_context(|| format!("unknown provider '{provider}'"))?;
    let model = validate_provider_token("model", model)?;
    let id = preset.id;
    let mut text = format!("provider={id}\nmodel={model}\n");
    if let Some(base_url) = nonempty(base_url) {
        let base_url = validate_provider_token("base_url", base_url)?;
        chat_completions_url(&base_url)?;
        text.push_str(&format!("base_url={base_url}\n"));
    } else if preset.default_base_url.is_none() {
        bail!("compatible provider requires a base URL");
    }
    Ok(text)
}

/// Writes `provider`, `model`, and an optional `base_url`. Never writes an API key.
pub fn write_provider_file(
    path: &std::path::Path,
    provider: &str,
    model: &str,
    base_url: Option<&str>,
) -> anyhow::Result<()> {
    let text = format_provider_file(provider, model, base_url)?;
    write_provider_text(path, text)
}

pub(crate) fn write_provider_text(path: &std::path::Path, text: String) -> anyhow::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    std::fs::write(path, text)
        .with_context(|| format!("failed to write provider file {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to set permissions on {}", path.display()))?;
    }
    Ok(())
}

fn validate_provider_token(key: &str, value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    if value.is_empty()
        || value.contains('=')
        || value
            .chars()
            .any(|ch| ch.is_whitespace() || ch.is_control())
    {
        bail!("{key} must be a single token");
    }
    let lower = value.to_ascii_lowercase();
    if lower.contains("api_key")
        || lower.contains("apikey")
        || lower.contains("token")
        || lower.contains("secret")
        || lower.starts_with("sk-")
        || lower.starts_with("sk_")
        || lower.starts_with("xai-")
        || lower.starts_with("gsk_")
        || lower.starts_with("bearer ")
    {
        bail!("{key} must not carry a secret; set {ENV_API_KEY} in the environment");
    }
    Ok(value.to_owned())
}

pub(crate) fn validate_env_var_name(value: &str) -> anyhow::Result<String> {
    let value = value.trim();
    let valid = !value.is_empty()
        && value.len() <= 64
        && value.chars().enumerate().all(|(index, ch)| {
            if index == 0 {
                ch.is_ascii_alphabetic() || ch == '_'
            } else {
                ch.is_ascii_alphanumeric() || ch == '_'
            }
        });
    if !valid {
        bail!(
            "name the environment variable that holds the API key, for example {ENV_API_KEY}; do not paste the key"
        );
    }
    Ok(value.to_owned())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ProviderFile {
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
    api_key_env: Option<String>,
}

pub(crate) fn parse_provider_file(text: &str) -> anyhow::Result<ProviderFile> {
    let mut file = ProviderFile::default();
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            bail!("provider file line {}: expected key=value", index + 1);
        };
        let key = key.trim();
        let value = trim_value(value);
        if value.is_empty() {
            continue;
        }
        match key {
            "provider" => file.provider = Some(value),
            "base_url" => file.base_url = Some(value),
            "model" => file.model = Some(value),
            "api_key_env" => file.api_key_env = Some(validate_env_var_name(&value)?),
            "api_key" | "apikey" | "token" | "secret" => {
                bail!(
                    "{key} must not be stored in the provider file; set {ENV_API_KEY} or the OS keychain"
                );
            }
            other => bail!("unknown provider file key '{other}'"),
        }
    }
    Ok(file)
}

fn read_provider_file() -> anyhow::Result<Option<ProviderFile>> {
    let Some(path) = provider_file_path() else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read provider file {}", path.display()))?;
    parse_provider_file(&text).map(Some)
}

fn provider_file_path() -> Option<std::path::PathBuf> {
    if let Some(path) = nonempty_owned(std::env::var(ENV_PROVIDER_FILE).ok()) {
        return Some(std::path::PathBuf::from(path));
    }
    let home = dirs_home()?;
    let path = home.join(".ownterm").join("provider.env");
    path.is_file().then_some(path)
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from))
}

fn env_or(name: &str, file_value: Option<String>) -> Option<String> {
    nonempty_owned(std::env::var(name).ok()).or(file_value)
}

fn env_nonempty(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .is_some_and(|value| !value.trim().is_empty())
}

fn nonempty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn nonempty_owned(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    })
}

fn trim_value(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .or_else(|| {
            trimmed
                .strip_prefix('\'')
                .and_then(|rest| rest.strip_suffix('\''))
        })
        .unwrap_or(trimmed)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ollama_preset_builds_loopback_chat_url() {
        let resolved = resolve(&ProviderInputs {
            provider: Some("ollama".into()),
            model: Some("llama3.3".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(resolved.provider_id, "ollama");
        assert_eq!(
            resolved.endpoint.as_str(),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
        assert!(resolved.api_key.is_none());
    }

    #[test]
    fn xai_preset_requires_https_and_api_key() {
        let missing = resolve(&ProviderInputs {
            provider: Some("xai".into()),
            model: Some("grok-3".into()),
            ..Default::default()
        });
        assert!(missing.unwrap_err().to_string().contains("OWNTERM_API_KEY"));

        let resolved = resolve(&ProviderInputs {
            provider: Some("XAI".into()),
            model: Some("grok-3".into()),
            api_key: Some("xai-test".into()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(resolved.provider_id, "xai");
        assert_eq!(
            resolved.endpoint.as_str(),
            "https://api.x.ai/v1/chat/completions"
        );
        assert_eq!(resolved.api_key.as_deref(), Some("xai-test"));
    }

    #[test]
    fn explicit_base_url_overrides_preset() {
        let resolved = resolve(&ProviderInputs {
            provider: Some("openai".into()),
            base_url: Some("https://example.test/v1".into()),
            model: Some("gpt-4o".into()),
            api_key: Some("sk-test".into()),
            api_key_env: None,
        })
        .unwrap();
        assert_eq!(
            resolved.endpoint.as_str(),
            "https://example.test/v1/chat/completions"
        );
    }

    #[test]
    fn rejects_plain_http_for_remote_hosts() {
        assert!(chat_completions_url("http://example.com/v1").is_err());
        assert!(chat_completions_url("http://127.0.0.1:11434/v1").is_ok());
        assert!(chat_completions_url("http://localhost:1234/v1").is_ok());
    }

    #[test]
    fn preserves_complete_chat_completions_url() {
        assert_eq!(
            chat_completions_url("https://example.com/v1/chat/completions")
                .unwrap()
                .as_str(),
            "https://example.com/v1/chat/completions"
        );
    }

    #[test]
    fn provider_file_rejects_secrets_and_unknown_keys() {
        assert!(parse_provider_file("api_key=sk-secret\n").is_err());
        assert!(parse_provider_file("token=abc\n").is_err());
        let file = parse_provider_file(
            "# local\nprovider = xai\nmodel = \"grok-3\"\nbase_url = 'https://api.x.ai/v1'\n",
        )
        .unwrap();
        assert_eq!(file.provider.as_deref(), Some("xai"));
        assert_eq!(file.model.as_deref(), Some("grok-3"));
        assert_eq!(file.base_url.as_deref(), Some("https://api.x.ai/v1"));
    }

    #[test]
    fn unknown_provider_is_an_error() {
        let err = resolve(&ProviderInputs {
            provider: Some("not-a-provider".into()),
            model: Some("m".into()),
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.to_string().contains("unknown OWNTERM_PROVIDER"));
    }

    #[test]
    fn write_provider_file_round_trips_without_a_secret() {
        let dir =
            std::env::temp_dir().join(format!("ownterm-provider-{}-roundtrip", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        let path = dir.join("provider.env");
        write_provider_file(
            &path,
            "Ollama",
            "llama3.3",
            Some("http://127.0.0.1:11434/v1"),
        )
        .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(!text.to_ascii_lowercase().contains("api_key"));
        let file = parse_provider_file(&text).unwrap();
        assert_eq!(file.provider.as_deref(), Some("ollama"));
        assert_eq!(file.model.as_deref(), Some("llama3.3"));
        assert_eq!(file.base_url.as_deref(), Some("http://127.0.0.1:11434/v1"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn write_provider_file_rejects_secrets_and_public_http() {
        assert!(format_provider_file("ollama", "api_key=sk", None).is_err());
        assert!(format_provider_file("ollama", "my-token", None).is_err());
        assert!(format_provider_file("ollama", "llama3.3", Some("http://example.com/v1")).is_err());
        assert!(format_provider_file("compatible", "m", None).is_err());
        assert!(format_provider_file("nope", "m", None).is_err());
    }

    #[test]
    fn suggested_models_and_catalog_url_stay_local() {
        assert_eq!(
            suggested_models("ollama"),
            &["llama3.3", "qwen2.5-coder:7b"]
        );
        assert!(suggested_models("compatible").is_empty());
        assert!(suggested_models("not-a-provider").is_empty());
        assert_eq!(
            models_list_url("http://127.0.0.1:11434/v1")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:11434/v1/models"
        );
        assert_eq!(
            models_list_url_for_provider("xai", None).unwrap().as_str(),
            "https://api.x.ai/v1/models"
        );
        let ids = parse_model_ids(r#"{"data":[{"id":"llama3.3"},{"id":"  "},{}]}"#).unwrap();
        assert_eq!(ids, vec!["llama3.3".to_owned()]);
        assert!(parse_model_ids("not-json").is_err());
    }
}
