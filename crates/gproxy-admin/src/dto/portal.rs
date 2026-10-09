use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::UserKeyPrefix;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalContextDto {
    pub user_name: String,
    pub recent_requests_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalSessionStatusDto {
    pub user: Option<PortalContextDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalPasswordChangeRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalKeyCreateRequest {
    pub prefix: UserKeyPrefix,
    pub label: Option<String>,
    pub expires_at: Option<i64>,
}

/// Which models one API key may reach. Empty `allowed` means the key has no
/// row of its own and inherits the account's access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalKeyModelsDto {
    pub available: Vec<String>,
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalKeyModelsRequest {
    pub models: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalSettingsDto {
    pub recent_requests_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalModelCapabilityDto {
    pub source: String,
    pub operation: String,
    pub group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalModelDto {
    pub name: String,
    pub capabilities: Vec<PortalModelCapabilityDto>,
}

/// A single upstream source backing an exposed model. The portal manages these
/// without exposing the underlying route concept.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalModelSourceDto {
    pub id: i64,
    pub provider_id: i64,
    pub upstream_model: String,
    pub tier: u32,
    pub weight: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalModelSourceWriteRequest {
    pub provider_id: i64,
    pub upstream_model: String,
    pub tier: u32,
    pub weight: u32,
    pub enabled: bool,
}

/// An exposed model together with every source that serves it. The route that
/// ties them together is an implementation detail the portal never sees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalManagedModelDto {
    pub id: i64,
    pub name: String,
    pub display_name: Option<String>,
    pub enabled: bool,
    pub sources: Vec<PortalModelSourceDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalManagedModelWriteRequest {
    pub name: String,
    pub display_name: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalUsageQueryDto {
    pub from: i64,
    pub to: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalUsageDto {
    pub from: i64,
    pub to: i64,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalUsageOverviewQueryDto {
    pub from: i64,
    pub to: i64,
    pub user_key_id: Option<i64>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalUsagePointDto {
    pub bucket_start: i64,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalUsageModelDto {
    pub model: String,
    /// Which upstream served it: the same public model can fan out across
    /// providers, and their usage is not the same usage.
    pub provider: Option<String>,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalUsageOverviewDto {
    pub from: i64,
    pub to: i64,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: String,
    pub series: Vec<PortalUsagePointDto>,
    pub models: Vec<PortalUsageModelDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum PortalQuotaScopeDto {
    User,
    Organization,
    Team,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(rename_all = "snake_case")]
pub enum PortalQuotaWindowKindDto {
    Total,
    Daily,
    Weekly,
    Monthly,
    #[serde(rename = "5h")]
    #[ts(rename = "5h")]
    FiveHour,
    #[serde(rename = "7d")]
    #[ts(rename = "7d")]
    SevenDay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalQuotaWindowDto {
    pub scope: PortalQuotaScopeDto,
    pub window_kind: PortalQuotaWindowKindDto,
    pub window_start: Option<i64>,
    pub reset_at: Option<i64>,
    pub started: bool,
    pub cost_used: String,
    pub cost_limit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalRecentQueryDto {
    pub limit: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalRecentRequestDto {
    pub request_id: String,
    pub at: i64,
    pub provider_name: Option<String>,
    pub operation: Option<String>,
    pub upstream_model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub cost: String,
    pub usage_source: String,
    pub ended: String,
    pub latency_ms: u64,
}

/// One registered upstream MCP server, as the portal lists it. `tools` is the
/// last discovered catalogue; `tools_fetched_at` lets the UI show its age.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalMcpServerDto {
    pub id: i64,
    pub name: String,
    pub url: String,
    pub auth_kind: String,
    pub auth_header_name: Option<String>,
    pub enabled: bool,
    pub tools: Vec<String>,
    pub tools_fetched_at: Option<i64>,
}

/// `token` is write-only: the portal never reads a credential back, so an
/// update that leaves it `None` keeps the stored one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalMcpServerWriteRequest {
    pub name: String,
    pub url: String,
    pub auth_kind: String,
    pub auth_header_name: Option<String>,
    pub token: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalMcpDiscoverResponse {
    pub ok: bool,
    pub tools: Vec<String>,
    pub message: Option<String>,
}

/// One server as the grant editor sees it. Disabled servers are listed too:
/// a key may still hold a grant on one, and hiding it would silently drop that
/// grant on the next save.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalMcpServerToolsDto {
    pub name: String,
    pub enabled: bool,
    pub tools: Vec<String>,
}

/// One server's grant: the tool-name patterns this key may call. `*` allows
/// every tool the server advertises.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalKeyMcpGrantDto {
    pub server: String,
    pub patterns: Vec<String>,
}

/// Which MCP tools one API key may call. Unlike models there is no account
/// fallback: an empty `allowed` means this key reaches no MCP tools at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalKeyMcpDto {
    pub available: Vec<PortalMcpServerToolsDto>,
    pub allowed: Vec<PortalKeyMcpGrantDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PortalKeyMcpRequest {
    pub grants: Vec<PortalKeyMcpGrantDto>,
}

/// A live snapshot of what this account's keys are doing right now.
///
/// Everything is scoped to the asking user and a short window (five minutes):
/// "active" means traffic in that window, and `in_flight` is the requests
/// moving through the gateway at the instant of the call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PortalActivityDto {
    pub now: i64,
    pub in_flight: u64,
    pub window_seconds: i64,
    pub last_minute_requests: u64,
    pub last_minute_tokens: u64,
    pub requests: Vec<PortalActivityRequestDto>,
    /// Just-finished requests, newest first — so the list still reads as
    /// activity between polls, when nothing happens to be in flight.
    pub recent: Vec<PortalActivityRecentDto>,
    pub sessions: Vec<PortalActivitySessionDto>,
    pub models: Vec<PortalActivityModelDto>,
}

/// One request that finished recently: what ran, on which key, how long it took.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PortalActivityRecentDto {
    pub request_id: String,
    pub key_id: i64,
    pub prefix: Option<String>,
    pub label: Option<String>,
    pub model: String,
    pub provider: Option<String>,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub at: i64,
    pub latency_ms: u64,
}

/// One request moving through the gateway right now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalActivityRequestDto {
    pub request_id: String,
    pub key_id: i64,
    pub prefix: Option<String>,
    pub label: Option<String>,
    pub model: Option<String>,
    /// When the gateway admitted it, epoch milliseconds.
    pub started_at: i64,
}

/// One API key with traffic in the window — the portal's notion of an
/// "active session": a client that is talking, which models it uses, and when
/// it was last seen.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
pub struct PortalActivitySessionDto {
    pub key_id: i64,
    pub prefix: Option<String>,
    pub label: Option<String>,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub last_at: i64,
    pub models: Vec<String>,
    /// Generated-token throughput: the key's output tokens over the last
    /// minute, divided by 60. Zero when the key wrote nothing recently.
    pub tokens_per_second: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PortalActivityModelDto {
    pub model: String,
    pub provider: Option<String>,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub last_at: i64,
}
