use super::{validate_component, PathError};

const VERBATIM_PREFIX: &str = r"\\?\";
const VERBATIM_FORWARD_PREFIX: &str = "//?/";
const UNC_TAG: &str = "UNC";
const DRIVE_PREFIX_LENGTH: usize = 2;

#[derive(Debug, PartialEq, Eq)]
enum Anchor {
    Unix,
    Drive(char),
    Share(String, String),
}

pub(super) struct Absolute<'a> {
    anchor: Anchor,
    components: Vec<&'a str>,
}

impl<'a> Absolute<'a> {
    pub(super) fn parse(value: &'a str) -> Result<Self, PathError> {
        if let Some(body) = value
            .strip_prefix(VERBATIM_PREFIX)
            .or_else(|| value.strip_prefix(VERBATIM_FORWARD_PREFIX))
        {
            if body
                .get(..UNC_TAG.len())
                .is_some_and(|tag| tag.eq_ignore_ascii_case(UNC_TAG))
                && body
                    .get(UNC_TAG.len()..)
                    .is_some_and(|tail| tail.starts_with(['/', '\\']))
            {
                return Self::share(&body[UNC_TAG.len() + 1..]);
            }
            return Self::drive(body);
        }
        if let Some(body) = value
            .strip_prefix(r"\\")
            .or_else(|| value.strip_prefix("//"))
        {
            return Self::share(body);
        }
        if value.as_bytes().get(1) == Some(&b':') {
            return Self::drive(value);
        }
        if value.starts_with('/') {
            let components: Vec<_> = value.split('/').filter(|part| !part.is_empty()).collect();
            for component in &components {
                validate_component(component)?;
            }
            return Ok(Self {
                anchor: Anchor::Unix,
                components,
            });
        }
        Err(PathError::InvalidRoot)
    }

    fn drive(value: &'a str) -> Result<Self, PathError> {
        let bytes = value.as_bytes();
        if !bytes.first().is_some_and(u8::is_ascii_alphabetic) || bytes.get(1) != Some(&b':') {
            return Err(PathError::InvalidRoot);
        }
        let tail = value
            .get(DRIVE_PREFIX_LENGTH..)
            .ok_or(PathError::InvalidRoot)?;
        if !tail.starts_with(['/', '\\']) {
            return Err(PathError::InvalidRoot);
        }
        let components = windows_components(tail)?;
        Ok(Self {
            anchor: Anchor::Drive(char::from(bytes[0]).to_ascii_uppercase()),
            components,
        })
    }

    fn share(body: &'a str) -> Result<Self, PathError> {
        let parts = windows_components(body)?;
        let (Some(server), Some(share)) = (parts.first(), parts.get(1)) else {
            return Err(PathError::InvalidRoot);
        };
        Ok(Self {
            anchor: Anchor::Share(server.to_ascii_lowercase(), share.to_ascii_lowercase()),
            components: parts[2..].to_vec(),
        })
    }

    pub(super) fn suffix_in(&self, root: &Self) -> Option<&[&'a str]> {
        if self.anchor != root.anchor || self.components.len() < root.components.len() {
            return None;
        }
        let same =
            self.components
                .iter()
                .zip(&root.components)
                .all(|(actual, expected)| match self.anchor {
                    Anchor::Unix => actual == expected,
                    Anchor::Drive(_) | Anchor::Share(_, _) => actual.eq_ignore_ascii_case(expected),
                });
        same.then(|| &self.components[root.components.len()..])
    }
}

fn windows_components(value: &str) -> Result<Vec<&str>, PathError> {
    let components: Vec<_> = value
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .collect();
    for component in &components {
        validate_component(component)?;
    }
    Ok(components)
}
