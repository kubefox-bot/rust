use hmail_cleaner_center::config::{CenterArgs, CenterConfig};

#[test]
fn default_and_created_configurations_have_native_absolute_static_paths() {
    let root = tempfile::tempdir().unwrap();
    let args = CenterArgs {
        data_dir: root.path().to_path_buf(),
    };
    let defaults = CenterConfig::load(&args).expect("default configuration must load");
    assert!(defaults.static_dir.is_absolute());
    #[cfg(unix)]
    assert_eq!(defaults.static_dir, std::path::Path::new("/opt/hmail/web"));

    CenterConfig::create(
        root.path(),
        "https://localhost:8443",
        "0.0.0.0:8443".parse().unwrap(),
    )
    .unwrap();
    let saved = CenterConfig::load(&args).expect("created configuration must reload");
    assert_eq!(saved.static_dir, defaults.static_dir);
}
