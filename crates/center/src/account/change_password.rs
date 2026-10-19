use super::{AccountError, AccountService};
use crate::{
    auth::SessionLease,
    vault::{self, SystemEntropy},
};
use hmail_cleaner_protocol::browser::auth::ChangePasswordInput;
impl AccountService {
    pub async fn change_password(
        &self,
        lease: &SessionLease,
        input: ChangePasswordInput,
    ) -> Result<(), AccountError> {
        if input.new_password != input.confirm_password {
            return Err(AccountError::Invalid);
        }
        let account = self.current(lease).await?;
        let material = self
            .store
            .kdf
            .try_run(move || {
                let dek = vault::open_password(&account.material, &input.current_password)?;
                vault::replace_password(
                    &account.material,
                    &dek,
                    &input.new_password,
                    &SystemEntropy,
                )
            })
            .await
            .map_err(AccountError::from_kdf)?;
        self.commit(lease, material).await
    }
}
