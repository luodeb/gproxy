use std::path::PathBuf;

use ts_rs::{Config, TS};

use super::*;

#[test]
fn export_console_types() {
    let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../console/src/generated");
    if output.exists() {
        std::fs::remove_dir_all(&output).expect("clear generated TypeScript");
    }
    std::fs::create_dir_all(&output).expect("create generated TypeScript directory");
    let config = Config::new().with_out_dir(&output).with_large_int("number");
    macro_rules! export {
        ($($ty:ty),+ $(,)?) => {$({
            <$ty>::export_all(&config).expect(concat!("export ", stringify!($ty)));
        })+};
    }
    // Only the user portal ships in this fork's web bundle; the operator console
    // was replaced by the MCP control plane. Exporting the full admin DTO set
    // would regenerate ~200 unused files under `console/src/generated`.
    export!(
        ErrorEnvelope,
        OAuthSessionDto,
        OAuthSessionPageDto,
        OAuthAuthorizationRequest,
        OAuthConsentDto,
        OAuthAuthorizeDecision,
        OAuthRedirectDto,
        OAuthDeviceDecision,
        OAuthErrorDto,
        UserKeyDto,
        UserKeyCreateResponse,
        UserKeyRevealResponse,
        BoundarySourceDto,
        BoundaryConfidenceDto,
        QuotaCoverageDto,
        PortalContextDto,
        PortalLoginRequest,
        PortalSessionStatusDto,
        PortalPasswordChangeRequest,
        PortalKeyCreateRequest,
        PortalModelDto,
        PortalUsageQueryDto,
        PortalUsageDto,
        PortalQuotaWindowDto,
        PortalRecentQueryDto,
        PortalRecentRequestDto,
    );

    let mut names = std::fs::read_dir(&output)
        .expect("read generated TypeScript")
        .map(|entry| entry.expect("generated entry").file_name())
        .filter_map(|name| name.into_string().ok())
        .filter_map(|name| name.strip_suffix(".ts").map(str::to_owned))
        .filter(|name| name != "index")
        .collect::<Vec<_>>();
    names.sort();
    let index = names
        .into_iter()
        .map(|name| format!("export * from \"./{name}\";\n"))
        .collect::<String>();
    std::fs::write(output.join("index.ts"), index).expect("write generated TypeScript index");
}
