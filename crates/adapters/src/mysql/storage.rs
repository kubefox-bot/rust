use hmail_cleaner_core::cleanup::{MessageStorage, PortResult};

pub(super) fn resolve(
    account_id: i32,
    folder_id: i32,
    address: Option<&str>,
    folder_owner: Option<u32>,
) -> PortResult<MessageStorage> {
    let owner = u32::try_from(account_id).map_err(|_| "negative account ID")?;
    if folder_id <= 0 || folder_owner != Some(owner) {
        return Err("IMAP folder is absent or belongs to another account".into());
    }
    match account_id {
        0 => Ok(MessageStorage::Public),
        _ => address
            .map(|address| MessageStorage::Account {
                address: address.to_owned(),
            })
            .ok_or_else(|| "account is absent".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_requires_an_existing_public_folder() {
        assert_eq!(
            resolve(0, 40, None, Some(0)).unwrap(),
            MessageStorage::Public
        );
        for (account, folder, owner) in [
            (0, 0, Some(0)),
            (0, 40, None),
            (0, 10, Some(1)),
            (-1, 40, Some(0)),
        ] {
            assert!(resolve(account, folder, None, owner).is_err());
        }
    }

    #[test]
    fn account_requires_both_address_and_owned_folder() {
        assert!(resolve(1, 10, None, Some(1)).is_err());
        assert!(resolve(1, 40, Some("info@chem.orp"), Some(0)).is_err());
        assert_eq!(
            resolve(1, 10, Some("info@chem.orp"), Some(1)).unwrap(),
            MessageStorage::Account {
                address: "info@chem.orp".into()
            }
        );
    }
}
