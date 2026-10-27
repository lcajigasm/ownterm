use std::collections::HashSet;

use super::{FeatureFlag, apply_ownterm_feature_policy};

#[test]
fn ownterm_policy_keeps_local_agent_and_removes_business_features() {
    let mut flags = HashSet::from([
        FeatureFlag::AgentMode,
        FeatureFlag::APIKeyManagement,
        FeatureFlag::SoloUserByok,
        FeatureFlag::CloudMode,
        FeatureFlag::BillingAndUsagePageV2,
    ]);

    apply_ownterm_feature_policy(&mut flags);

    assert!(flags.contains(&FeatureFlag::AgentMode));
    assert!(flags.contains(&FeatureFlag::APIKeyManagement));
    assert!(flags.contains(&FeatureFlag::SoloUserByok));
    assert!(!flags.contains(&FeatureFlag::CloudMode));
    assert!(!flags.contains(&FeatureFlag::BillingAndUsagePageV2));
}

#[test]
fn ownterm_policy_enables_local_agent_features_from_an_empty_set() {
    let mut flags = HashSet::new();

    apply_ownterm_feature_policy(&mut flags);

    assert!(flags.contains(&FeatureFlag::AgentMode));
    assert!(flags.contains(&FeatureFlag::APIKeyManagement));
    assert!(flags.contains(&FeatureFlag::SoloUserByok));
}
