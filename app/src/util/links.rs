const OWNTERM_REPO_URL: &str = "https://github.com/lcajigasm/ownterm";

pub const USER_DOCS_URL: &str = "https://github.com/lcajigasm/ownterm/blob/master/docs/usage.md";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub const GITHUB_ISSUES_URL: &str = OWNTERM_REPO_URL;
pub const SLACK_URL: &str = OWNTERM_REPO_URL;
pub const PRIVACY_POLICY_URL: &str = OWNTERM_REPO_URL;

/// Issues are disabled on the OwnTerm repository, so feedback is directed at the repository itself.
pub fn feedback_form_url() -> String {
    OWNTERM_REPO_URL.to_owned()
}
