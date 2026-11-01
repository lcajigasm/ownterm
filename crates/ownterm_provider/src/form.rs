//! Settings form for `provider.env`.
//!
//! The saved file uses the same `provider`, `model`, and `base_url` lines as
//! `ownterm-tui configure-provider`. The API key is never written.

use std::path::Path;

use crate::{
    ENV_API_KEY, find_preset, format_provider_file, parse_provider_file, suggested_models,
    validate_env_var_name, write_provider_text,
};

/// Values edited in OSS Settings. `api_key_env` is a variable name, not a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderForm {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key_env: String,
}

impl Default for ProviderForm {
    fn default() -> Self {
        let provider = "ollama";
        Self {
            provider: provider.to_owned(),
            model: suggested_models(provider)
                .first()
                .copied()
                .unwrap_or("")
                .to_owned(),
            base_url: find_preset(provider)
                .and_then(|preset| preset.default_base_url)
                .unwrap_or("")
                .to_owned(),
            api_key_env: ENV_API_KEY.to_owned(),
        }
    }
}

/// Model ids offered for `provider`, the same list `ownterm-tui models` prints.
pub fn model_choices(provider: &str) -> &'static [&'static str] {
    suggested_models(provider)
}

pub fn provider_form_to_text(form: &ProviderForm) -> Result<String, String> {
    let provider = form.provider.trim();
    let preset = find_preset(provider).ok_or_else(|| "Choose a provider.".to_owned())?;
    let model = form.model.trim();
    if model.is_empty() {
        return Err("Choose a model.".to_owned());
    }
    let env_name = validate_env_var_name(form.api_key_env.trim()).map_err(|err| err.to_string())?;
    let base = form.base_url.trim();
    let base_arg = if base.is_empty() || preset.default_base_url == Some(base) {
        None
    } else {
        Some(base)
    };
    let mut text =
        format_provider_file(preset.id, model, base_arg).map_err(|err| err.to_string())?;
    if env_name != ENV_API_KEY {
        text.push_str(&format!("api_key_env={env_name}\n"));
    }
    if text.contains("sk-") || text.lines().any(|line| line.starts_with("api_key=")) {
        return Err("The provider file must not contain an API key.".to_owned());
    }
    Ok(text)
}

pub fn provider_form_from_text(text: &str) -> Result<ProviderForm, String> {
    let file = parse_provider_file(text).map_err(|err| err.to_string())?;
    let form = ProviderForm {
        provider: file.provider.unwrap_or_default(),
        model: file.model.unwrap_or_default(),
        base_url: file.base_url.unwrap_or_default(),
        api_key_env: file.api_key_env.unwrap_or_else(|| ENV_API_KEY.to_owned()),
    };
    provider_form_to_text(&form)?;
    Ok(form)
}

pub fn save_provider_form(path: &Path, form: &ProviderForm) -> Result<(), String> {
    let text = provider_form_to_text(form)?;
    write_provider_text(path, text).map_err(|err| err.to_string())
}

pub fn load_provider_form(path: &Path) -> Result<Option<ProviderForm>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(path)
        .map_err(|err| format!("Couldn't read {}: {err}", path.display()))?;
    provider_form_from_text(&text).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ownterm-form-{}-{name}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("provider.env")
    }

    #[test]
    fn save_matches_configure_provider_and_round_trips() {
        let path = temp_path("roundtrip");
        let form = ProviderForm {
            provider: "Ollama".into(),
            model: "llama3.3".into(),
            base_url: "http://127.0.0.1:11434/v1".into(),
            api_key_env: ENV_API_KEY.into(),
        };
        save_provider_form(&path, &form).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            text,
            format_provider_file("ollama", "llama3.3", None).unwrap()
        );
        assert!(!text.contains("api_key="));
        assert!(!text.contains("sk-"));
        let loaded = load_provider_form(&path).unwrap().unwrap();
        assert_eq!(loaded.provider, "ollama");
        assert_eq!(loaded.model, "llama3.3");
        assert_eq!(loaded.base_url, "");
        assert_eq!(loaded.api_key_env, ENV_API_KEY);
        assert_eq!(model_choices("ollama"), suggested_models("ollama"));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn custom_env_name_is_stored_and_a_pasted_key_is_rejected() {
        let path = temp_path("env-name");
        let form = ProviderForm {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            base_url: String::new(),
            api_key_env: "MY_OPENAI_KEY".into(),
        };
        save_provider_form(&path, &form).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("api_key_env=MY_OPENAI_KEY\n"));
        assert!(!text.contains("sk-"));
        let loaded = load_provider_form(&path).unwrap().unwrap();
        assert_eq!(loaded.api_key_env, "MY_OPENAI_KEY");

        let pasted = ProviderForm {
            api_key_env: "sk-live-secret".into(),
            ..form
        };
        let err = save_provider_form(&path, &pasted).unwrap_err();
        assert!(err.contains("do not paste the key"));
        assert!(!std::fs::read_to_string(&path).unwrap().contains("sk-live"));
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    #[test]
    fn validation_errors_name_the_field() {
        assert!(
            provider_form_to_text(&ProviderForm {
                provider: "ollama".into(),
                model: String::new(),
                base_url: String::new(),
                api_key_env: ENV_API_KEY.into(),
            })
            .unwrap_err()
            .contains("Choose a model")
        );
        assert!(
            provider_form_to_text(&ProviderForm {
                provider: "compatible".into(),
                model: "local-model".into(),
                base_url: String::new(),
                api_key_env: ENV_API_KEY.into(),
            })
            .unwrap_err()
            .contains("base URL")
        );
        assert!(
            provider_form_to_text(&ProviderForm {
                provider: "ollama".into(),
                model: "llama3.3".into(),
                base_url: "http://example.com/v1".into(),
                api_key_env: ENV_API_KEY.into(),
            })
            .is_err()
        );
        assert!(provider_form_from_text("api_key=sk-secret\n").is_err());
        assert!(load_provider_form(Path::new("/tmp/ownterm-form-missing-provider.env")).is_ok());
    }
}
