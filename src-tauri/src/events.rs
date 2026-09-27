use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Watch,
    Ask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Started,
    Prompt,
    Step,
    Blocked,
    NeedsYou,
    ReplyDelta,
    Done,
    Failed,
    Ended,
    /// Ask only: the run skips an untrusted folder's project settings (design v1.0 D6). A mini-chat notice; it
    /// never changes a thread.
    Untrusted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetEvent {
    pub session_id: String,
    pub project: String,
    pub source: Source,
    pub kind: Kind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub at: i64,
}
