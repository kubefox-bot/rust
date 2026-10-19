use std::path::Path;

pub fn assert_absent(directory: &Path, secrets: &[&str]) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        let metadata = path.metadata().unwrap();
        // Empty files contain no plaintext. In particular, Windows prevents
        // reading the empty center.lock while its exclusive lease is active.
        if !metadata.is_file() || metadata.len() == 0 {
            continue;
        }
        let bytes = std::fs::read(&path)
            .unwrap_or_else(|error| panic!("cannot inspect {}: {error}", path.display()));
        for secret in secrets {
            assert!(
                !bytes
                    .windows(secret.len())
                    .any(|value| value == secret.as_bytes()),
                "plaintext secret persisted in {}",
                path.display()
            );
        }
    }
}
