use std::{io, time::Duration};

const PAUSES: [Duration; 2] = [Duration::from_millis(100), Duration::from_millis(300)];
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_LOCK_VIOLATION: i32 = 33;

pub(super) fn windows_lock(error: &io::Error) -> bool {
    matches!(
        error.raw_os_error(),
        Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
    )
}

pub(super) fn run<T, E>(
    mut operation: impl FnMut() -> Result<T, E>,
    retryable: impl Fn(&E) -> bool,
    mut pause: impl FnMut(Duration),
) -> Result<T, E> {
    let mut retry = 0;
    loop {
        let outcome = operation();
        match &outcome {
            Err(error) if retry < PAUSES.len() && retryable(error) => {
                pause(PAUSES[retry]);
                retry += 1;
            }
            _ => return outcome,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io, time::Duration};

    #[test]
    fn temporary_lock_has_three_attempts() {
        let mut attempts = 0;
        let mut pauses = Vec::new();
        let result: io::Result<()> = run(
            || {
                attempts += 1;
                Err(io::Error::from_raw_os_error(32))
            },
            windows_lock,
            |delay| pauses.push(delay),
        );
        assert!(result.is_err());
        assert_eq!(attempts, 3);
        assert_eq!(
            pauses,
            [Duration::from_millis(100), Duration::from_millis(300)]
        );
    }
    #[test]
    fn permanent_denial_has_one_attempt() {
        let mut attempts = 0;
        let result: io::Result<()> = run(
            || {
                attempts += 1;
                Err(io::Error::from_raw_os_error(5))
            },
            windows_lock,
            |_| panic!("permanent error must not sleep"),
        );
        assert!(result.is_err());
        assert_eq!(attempts, 1);
    }
    #[test]
    fn temporary_failure_then_success_finishes_operation() {
        let mut attempts = 0;
        let result = run(
            || {
                attempts += 1;
                if attempts == 1 {
                    Err(io::Error::from_raw_os_error(33))
                } else {
                    Ok(7)
                }
            },
            windows_lock,
            |_| {},
        );
        assert_eq!(result.unwrap(), 7);
        assert_eq!(attempts, 2);
    }
}
