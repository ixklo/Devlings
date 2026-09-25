use serde::Serialize;
use serde_json::Value;

pub const SCRUBBED_ENV: [&str; 6] = [
    "ANTHROPIC_API_KEY",
    "ANTHROPIC_AUTH_TOKEN",
    "ANTHROPIC_BASE_URL",
    "CLAUDE_CODE_USE_BEDROCK",
    "CLAUDE_CODE_USE_VERTEX",
    "CLAUDE_CODE_USE_FOUNDRY",
];

const SUBSCRIPTIONS: [&str; 4] = ["pro", "max", "team", "enterprise"];
const LOGIN_HINT: &str = "Run `claude auth login` and choose your Claude account.";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthVerdict {
    Allowed { subscription: String },
    Refused { reason: String },
}

pub fn clean_env<I: IntoIterator<Item = (String, String)>>(vars: I) -> Vec<(String, String)> {
    vars.into_iter()
        .filter(|(k, _)| !SCRUBBED_ENV.iter().any(|s| s.eq_ignore_ascii_case(k)))
        .collect()
}

pub fn verdict_from_auth_status(stdout: &str, exit_ok: bool) -> AuthVerdict {
    let Ok(v) = serde_json::from_str::<Value>(stdout.trim()) else {
        return AuthVerdict::Refused { reason: "Couldn't read Claude Code's login status.".into() };
    };
    if !exit_ok || v.get("loggedIn").and_then(Value::as_bool) != Some(true) {
        return AuthVerdict::Refused { reason: format!("Claude Code isn't logged in. {LOGIN_HINT}") };
    }
    let field = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("");
    let subscription = field("subscriptionType").to_ascii_lowercase();
    if field("authMethod") == "claude.ai"
        && field("apiProvider") == "firstParty"
        && SUBSCRIPTIONS.contains(&subscription.as_str())
    {
        AuthVerdict::Allowed { subscription }
    } else {
        AuthVerdict::Refused {
            reason: format!("Ask only works with a Claude subscription login (Pro, Max, Team or Enterprise). {LOGIN_HINT}"),
        }
    }
}

pub fn init_allowed(api_key_source: &str) -> bool {
    api_key_source == "none"
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real `claude auth status` output from 2.1.282, with personal fields redacted.
    const PRO: &str = r#"{
  "loggedIn": true,
  "authMethod": "claude.ai",
  "apiProvider": "firstParty",
  "analyticsDisabled": false,
  "projectsDirectory": "C:\\Users\\u\\.claude\\projects",
  "configDirectory": "C:\\Users\\u\\.claude",
  "email": "user@example.com",
  "orgId": "00000000-0000-0000-0000-000000000000",
  "orgName": "user@example.com's Organization",
  "subscriptionType": "pro"
}"#;

    #[test]
    fn scrubs_paid_auth_env() {
        let vars = vec![
            ("PATH".to_string(), "x".to_string()),
            ("ANTHROPIC_API_KEY".to_string(), "sk".to_string()),
            ("anthropic_auth_token".to_string(), "t".to_string()),
            ("ANTHROPIC_BASE_URL".to_string(), "u".to_string()),
            ("CLAUDE_CODE_USE_BEDROCK".to_string(), "1".to_string()),
            ("CLAUDE_CODE_USE_VERTEX".to_string(), "1".to_string()),
            ("CLAUDE_CODE_USE_FOUNDRY".to_string(), "1".to_string()),
            ("HOME".to_string(), "h".to_string()),
        ];
        let kept: Vec<String> = clean_env(vars).into_iter().map(|(k, _)| k).collect();
        assert_eq!(kept, vec!["PATH", "HOME"]);
    }

    #[test]
    fn allows_subscription_logins() {
        assert_eq!(verdict_from_auth_status(PRO, true), AuthVerdict::Allowed { subscription: "pro".into() });
        let max = PRO.replace("\"pro\"", "\"max\"");
        assert_eq!(verdict_from_auth_status(&max, true), AuthVerdict::Allowed { subscription: "max".into() });
    }

    #[test]
    fn refuses_everything_else() {
        let refused = |s: &str, ok: bool| matches!(verdict_from_auth_status(s, ok), AuthVerdict::Refused { .. });
        assert!(refused(r#"{"loggedIn":true,"authMethod":"api_key","apiProvider":"firstParty"}"#, true));
        assert!(refused(&PRO.replace("firstParty", "bedrock"), true));
        assert!(refused(&PRO.replace("\"pro\"", "\"free\""), true));
        assert!(refused(r#"{"loggedIn":false}"#, false));
        assert!(refused("not json", true));
        assert!(refused(PRO, false));
        match verdict_from_auth_status(r#"{"loggedIn":false}"#, false) {
            AuthVerdict::Refused { reason } => assert!(reason.contains("isn't logged in"), "{reason}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn init_must_use_subscription() {
        assert!(init_allowed("none"));
        assert!(!init_allowed("ANTHROPIC_API_KEY"));
        assert!(!init_allowed("apiKeyHelper"));
        assert!(!init_allowed(""));
    }

    #[test]
    fn verdict_serializes_for_frontend() {
        let v = serde_json::to_value(AuthVerdict::Refused { reason: "r".into() }).unwrap();
        assert_eq!(v, serde_json::json!({"status": "refused", "reason": "r"}));
    }
}
