// Isolated SCM test host; never controls hMailServer or opens a database.
use hmail_cleaner_adapters::service;
use std::time::Duration;
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 || !args[1].starts_with("hMailCleanerFixture_") {
        std::process::exit(2);
    }
    let fail = args[2] == "fail";
    let result = service::host(
        &args[1],
        Box::new(move |stop| {
            if fail {
                return Err("intentional SCM fixture failure".into());
            }
            while !stop.is_requested() {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(())
        }),
    );
    if result.is_err() {
        std::process::exit(1);
    }
}
