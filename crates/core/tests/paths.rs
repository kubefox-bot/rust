use chrono::NaiveDateTime;
use hmail_cleaner_core::{
    cleanup::{Candidate, MessageStorage},
    paths::resolve_path,
};
use std::path::{Path, PathBuf};

const NAME: &str = "{6C2CD4A9-683F-4EB4-91D7-9E2FD3700000}.eml";

#[test]
fn reference_guid_accepts_basename_and_legacy_paths_but_rejects_corruption() {
    use hmail_cleaner_core::paths::message_guid;
    assert_eq!(
        message_guid(NAME).unwrap().to_string(),
        "6c2cd4a9-683f-4eb4-91d7-9e2fd3700000"
    );
    assert_eq!(
        message_guid(&format!(r"D:\Data\domain\account\6C\{NAME}")).unwrap(),
        message_guid(NAME).unwrap()
    );
    for invalid in [
        "bad.eml",
        "",
        "{not-a-guid}.eml",
        "{6C2CD4A9-683F-4EB4-91D7-9E2FD3700000}.txt",
    ] {
        assert!(message_guid(invalid).is_err());
    }
}

fn message(address: &str, filename: &str) -> Candidate {
    Candidate {
        id: 1,
        account_id: 2,
        folder_id: 3,
        storage: MessageStorage::Account {
            address: address.into(),
        },
        filename: filename.into(),
        created_at: NaiveDateTime::parse_from_str("2026-09-01 10:00:00", "%Y-%m-%d %H:%M:%S")
            .unwrap(),
    }
}

#[test]
fn guid_resolves_to_account_bucket() {
    assert_eq!(
        resolve_path(Path::new("/data"), &message("info@chem.orp", NAME)).unwrap(),
        PathBuf::from("/data/chem.orp/info/6C").join(NAME)
    );
}

#[test]
fn public_guid_resolves_without_an_account_directory() {
    let public = Candidate {
        account_id: 0,
        storage: MessageStorage::Public,
        ..message("unused", NAME)
    };
    assert_eq!(
        resolve_path(Path::new("/data"), &public).unwrap(),
        PathBuf::from("/data/#Public/6C").join(NAME)
    );
    let legacy = Candidate {
        filename: format!(r"D:\Data\#Public\6C\{NAME}"),
        ..public
    };
    assert_eq!(
        resolve_path(Path::new(r"D:\Data"), &legacy).unwrap(),
        Path::new(r"D:\Data").join("#Public").join("6C").join(NAME)
    );
    assert!(resolve_path(
        Path::new("/data"),
        &Candidate {
            filename: format!("../{NAME}"),
            ..legacy
        }
    )
    .is_err());
}

#[test]
fn legacy_absolute_path_stays_in_root() {
    let root = Path::new("/data");
    let absolute = format!("/data/chem.orp/info/6C/{NAME}");
    assert_eq!(
        resolve_path(root, &message("info@chem.orp", &absolute)).unwrap(),
        PathBuf::from(absolute)
    );
}

#[test]
fn windows_legacy_paths_are_checked_on_mac() {
    let path = format!("d:\\data\\chem.orp\\info\\6C\\{NAME}");
    let result = resolve_path(Path::new("D:\\Data"), &message("info@chem.orp", &path)).unwrap();
    assert_eq!(
        result,
        Path::new("D:\\Data")
            .join("chem.orp")
            .join("info")
            .join("6C")
            .join(NAME)
    );
}

#[test]
fn similar_root_is_rejected() {
    assert!(resolve_path(
        Path::new("/data"),
        &message(
            "info@chem.orp",
            &format!("/data-other/chem.orp/info/6C/{NAME}")
        )
    )
    .is_err());
}

#[test]
fn traversal_stream_and_drive_relative_are_rejected() {
    for filename in [
        format!("../{NAME}"),
        format!("D:{NAME}"),
        format!("{NAME}:hidden"),
    ] {
        assert!(
            resolve_path(Path::new("/data"), &message("info@chem.orp", &filename)).is_err(),
            "{filename}"
        );
    }
}

#[test]
fn unicode_and_plus_account_are_preserved() {
    assert_eq!(
        resolve_path(Path::new("/data"), &message("почта+tag@chem.orp", NAME)).unwrap(),
        PathBuf::from("/data/chem.orp/почта+tag/6C").join(NAME)
    );
}
