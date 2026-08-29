use serde::Serialize;
use tauri::State;
use vault_app::{VaultApplication, VaultSummary};

/// Tauri IPC 只返回界面启动所需的计数和版本，不暴露密码库内容。
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct VaultStatusDto {
    schema_version: u32,
    revision: u64,
    category_count: usize,
    account_count: usize,
    initialized: bool,
}

impl From<VaultSummary> for VaultStatusDto {
    fn from(summary: VaultSummary) -> Self {
        Self {
            schema_version: summary.schema_version,
            revision: summary.revision,
            category_count: summary.category_count,
            account_count: summary.account_count,
            initialized: summary.initialized,
        }
    }
}

#[tauri::command]
pub(crate) fn load_vault(application: State<'_, VaultApplication>) -> VaultStatusDto {
    application.summary().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_contains_only_the_confirmed_startup_summary() {
        let dto = VaultStatusDto::from(VaultSummary {
            schema_version: 1,
            revision: 0,
            category_count: 0,
            account_count: 0,
            initialized: true,
        });

        assert_eq!(
            dto,
            VaultStatusDto {
                schema_version: 1,
                revision: 0,
                category_count: 0,
                account_count: 0,
                initialized: true,
            }
        );
    }
}
