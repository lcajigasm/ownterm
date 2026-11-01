use super::{
    Access, AttemptFailure, CatalogModel, TaskClass, catalog_models, classify, classify_failure,
    loopback_port_open, next_candidate, normalize_pin, priced_on, rank, reason_line,
    should_fallback,
};

fn model(provider: &str, id: &str, input: i64, output: i64) -> CatalogModel {
    CatalogModel {
        provider: provider.to_owned(),
        id: id.to_owned(),
        input_micros: input,
        output_micros: output,
        price_exact: true,
        context_tokens: 32_768,
        tools: true,
        vision: false,
        reasoning: false,
        code: true,
        quality: 3,
        api_key_env: (input > 0).then(|| "OWNTERM_TEST_KEY".to_owned()),
        source: "test".to_owned(),
    }
}

fn access(provider: &str, key_present: bool, reachable: bool) -> Access {
    Access {
        provider: provider.to_owned(),
        key_present,
        reachable,
    }
}

#[test]
fn classifier_is_deterministic() {
    let prompts = [
        "ls -la",
        "git status",
        "how do I list files",
        "hello",
        "refactor the parser and then run the tests",
        "write a migration",
    ];
    for prompt in prompts {
        assert_eq!(classify(prompt), classify(prompt));
    }
}

#[test]
fn classifier_covers_each_task() {
    assert_eq!(classify("ls -la"), TaskClass::Command);
    assert_eq!(classify("git status"), TaskClass::Command);
    assert_eq!(classify("how do I list files"), TaskClass::Command);
    assert_eq!(classify("hello"), TaskClass::Chat);
    assert_eq!(classify("what is a shell"), TaskClass::Chat);
    assert_eq!(
        classify("refactor the parser and then run the tests"),
        TaskClass::Agent
    );
    assert_eq!(classify("write a migration"), TaskClass::Agent);
    let steps = "plan\n1. read\n2. edit";
    assert_eq!(classify(steps), TaskClass::Agent);
    let mut long = String::new();
    for _ in 0..90 {
        long.push_str("line of context\n");
    }
    assert_eq!(classify(&long), TaskClass::LongContext);
    let mut fence = String::from("```\n");
    for _ in 0..45 {
        fence.push_str("code\n");
    }
    fence.push_str("```\n");
    assert_eq!(classify(&fence), TaskClass::LongContext);
}

#[test]
fn command_does_not_swallow_an_implementation_question() {
    assert_eq!(
        classify("how do I implement a database from scratch"),
        TaskClass::Agent
    );
}

#[test]
fn cheapest_local_beats_a_priced_model() {
    let models = vec![
        {
            let mut wide = model("openai", "gpt-4o-mini", 150_000, 600_000);
            wide.context_tokens = 128_000;
            wide
        },
        model("ollama", "llama3.3", 0, 0),
    ];
    let access = vec![access("openai", true, false), access("ollama", false, true)];
    let ranked = rank(TaskClass::Chat, 5, &models, &access, None).unwrap();
    assert_eq!(ranked[0].provider, "ollama");
    assert_eq!(ranked[0].model, "llama3.3");
    assert!(ranked.iter().any(|item| item.provider == "openai"));
}

#[test]
fn agent_skips_a_model_without_tools() {
    let models = vec![
        {
            let mut bare = model("localai", "llama3.3", 0, 0);
            bare.tools = false;
            bare
        },
        model("ollama", "qwen", 0, 0),
        {
            let mut strong = model("openai", "gpt-4o", 2_500_000, 10_000_000);
            strong.quality = 4;
            strong.context_tokens = 128_000;
            strong
        },
    ];
    let access = vec![
        access("localai", false, true),
        access("ollama", false, true),
        access("openai", true, false),
    ];
    let ranked = rank(TaskClass::Agent, 40, &models, &access, None).unwrap();
    assert_eq!(ranked[0].model, "qwen");
    assert!(ranked.iter().all(|item| item.model != "llama3.3"));
}

#[test]
fn small_context_is_excluded_for_a_long_prompt() {
    let models = vec![
        {
            let mut small = model("ollama", "small", 0, 0);
            small.context_tokens = 8_192;
            small
        },
        {
            let mut wide = model("openai", "wide", 150_000, 600_000);
            wide.context_tokens = 128_000;
            wide
        },
    ];
    let access = vec![access("ollama", false, true), access("openai", true, false)];
    let prompt_chars = 40_000;
    let ranked = rank(TaskClass::Chat, prompt_chars, &models, &access, None).unwrap();
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].model, "wide");
}

#[test]
fn nothing_available_is_an_error() {
    let models = vec![model("ollama", "llama3.3", 0, 0)];
    let access = vec![access("ollama", false, false)];
    let err = rank(TaskClass::Chat, 5, &models, &access, None).unwrap_err();
    assert!(err.contains("no available model"));
}

#[test]
fn equal_cost_prefers_higher_quality_then_the_configured_provider() {
    let models = vec![
        {
            let mut small = model("ollama", "small", 0, 0);
            small.quality = 2;
            small
        },
        {
            let mut better = model("lmstudio", "better", 0, 0);
            better.quality = 4;
            better
        },
        {
            let mut better = model("vllm", "better", 0, 0);
            better.quality = 4;
            better
        },
    ];
    let access = vec![
        access("ollama", false, true),
        access("lmstudio", false, true),
        access("vllm", false, true),
    ];
    let ranked = rank(TaskClass::Chat, 5, &models, &access, Some("vllm")).unwrap();
    assert_eq!(ranked[0].provider, "vllm");
    assert_eq!(ranked[0].quality, 4);
    assert_eq!(ranked.last().unwrap().model, "small");
}

#[test]
fn fallback_walks_the_list_and_stops() {
    let models = vec![model("ollama", "a", 0, 0), model("openai", "b", 100, 100)];
    let access = vec![access("ollama", false, true), access("openai", true, false)];
    let ranked = rank(TaskClass::Chat, 5, &models, &access, None).unwrap();
    let next = next_candidate(&ranked, &ranked[0]).unwrap();
    assert_eq!(next.model, "b");
    assert!(next_candidate(&ranked, next).is_none());
}

#[test]
fn every_failure_kind_falls_back() {
    for failure in [
        AttemptFailure::Error,
        AttemptFailure::RateLimit,
        AttemptFailure::Timeout,
    ] {
        assert!(should_fallback(failure));
    }
    assert_eq!(classify_failure(Some(429), ""), AttemptFailure::RateLimit);
    assert_eq!(classify_failure(Some(504), ""), AttemptFailure::Timeout);
    assert_eq!(
        classify_failure(None, "request timed out"),
        AttemptFailure::Timeout
    );
    assert_eq!(classify_failure(Some(500), "nope"), AttemptFailure::Error);
}

#[test]
fn reason_names_the_model_and_a_fallback() {
    let candidate = super::Candidate {
        provider: "ollama".into(),
        model: "llama3.3".into(),
        input_micros: 0,
        output_micros: 0,
        price_exact: true,
        quality: 3,
        context_tokens: 131_072,
    };
    let line = reason_line(TaskClass::Chat, &candidate, None);
    assert!(line.contains("ollama/llama3.3"));
    assert!(line.contains("short chat"));
    assert!(line.contains("$0"));
    let fell = reason_line(TaskClass::Chat, &candidate, Some("openai/gpt-4o-mini"));
    assert!(fell.contains("fell back"));
    assert!(fell.contains("openai/gpt-4o-mini"));
}

#[test]
fn cloud_endpoints_are_not_probed() {
    assert!(!loopback_port_open("https://api.openai.com/v1"));
    assert!(!loopback_port_open("not a url"));
}

#[test]
fn shipped_catalog_keeps_local_models_free_and_cites_sources() {
    assert_eq!(priced_on(), "2026-11-01");
    let models = catalog_models();
    assert!(models.len() >= 8);
    for model in models {
        assert!(!model.source.trim().is_empty(), "{}", model.id);
        assert!((1..=5).contains(&model.quality));
        if model.api_key_env.is_none() {
            assert_eq!(model.input_micros, 0);
            assert_eq!(model.output_micros, 0);
            assert!(model.price_exact);
        }
    }
    let mini = models
        .iter()
        .find(|model| model.provider == "openai" && model.id == "gpt-4o-mini")
        .unwrap();
    assert!(mini.price_exact);
    assert_eq!(mini.input_micros, 150_000);
    assert_eq!(mini.output_micros, 600_000);
    assert!(
        models
            .iter()
            .any(|model| model.provider == "groq" && !model.price_exact)
    );
    assert!(
        models
            .iter()
            .any(|model| model.provider == "xai" && !model.price_exact)
    );
}

#[test]
fn pin_parses_a_unique_id_and_rejects_ambiguity() {
    assert!(normalize_pin("").unwrap().is_none());
    assert!(normalize_pin("auto").unwrap().is_none());
    assert_eq!(
        normalize_pin("gpt-4o").unwrap(),
        Some(("openai".into(), "gpt-4o".into()))
    );
    assert_eq!(
        normalize_pin("openrouter/openai/gpt-4o-mini").unwrap(),
        Some(("openrouter".into(), "openai/gpt-4o-mini".into()))
    );
    assert!(
        normalize_pin("llama3.3")
            .unwrap_err()
            .contains("more than one")
    );
    assert!(normalize_pin("nope").is_err());
}
