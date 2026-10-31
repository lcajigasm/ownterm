//! Resolves an OpenAI-compatible chat endpoint for OwnTerm.
//!
//! API keys are read only from `OWNTERM_API_KEY`. A provider file may name the
//! provider, base URL, and model, and is rejected if it contains a secret.

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
    let inputs = ProviderInputs {
        provider: env_or(ENV_PROVIDER, file.as_ref().and_then(|f| f.provider.clone())),
        base_url: env_or(ENV_BASE_URL, file.as_ref().and_then(|f| f.base_url.clone())),
        model: env_or(ENV_MODEL, file.as_ref().and_then(|f| f.model.clone())),
        api_key: nonempty_owned(std::env::var(ENV_API_KEY).ok()),
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
        bail!("OWNTERM_API_KEY is required for the {id} provider");
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ProviderFile {
    provider: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
}

fn parse_provider_file(text: &str) -> anyhow::Result<ProviderFile> {
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
}
