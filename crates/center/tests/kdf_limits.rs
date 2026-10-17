use hmail_cleaner_center::vault::{
    self, AccountMaterial, KdfError, KdfGate, SecretText, SystemEntropy,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
const ARENA_BYTES: usize = 32 * 1024 * 1024;
static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static UNCLEARED: AtomicUsize = AtomicUsize::new(0);
struct ObservedAllocator;
// Only observes our exact-size test allocations; forwards allocation ownership to System.
unsafe impl GlobalAlloc for ObservedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == ARENA_BYTES {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        if layout.size() == ARENA_BYTES {
            // GlobalAlloc guarantees a live allocation of layout.size() until dealloc returns.
            if unsafe { std::slice::from_raw_parts(pointer, layout.size()) }
                .iter()
                .any(|byte| *byte != 0)
            {
                UNCLEARED.fetch_add(1, Ordering::SeqCst);
            }
        }
        unsafe { System.dealloc(pointer, layout) };
    }
}
#[global_allocator]
static ALLOCATOR: ObservedAllocator = ObservedAllocator;
#[test]
fn malformed_kdf_is_rejected_before_allocating_and_owned_arenas_are_wiped() {
    let password = SecretText::new("fixture-password-26!".into());
    let prepared = vault::initialize(&password, &SystemEntropy).unwrap();
    let before = ALLOCATIONS.load(Ordering::SeqCst);
    assert_eq!(
        before, 1,
        "initialization reuses one arena for both derivations"
    );
    let mut encoded = serde_json::to_value(&prepared.material).unwrap();
    let verifier = encoded["verifier"]
        .as_str()
        .unwrap()
        .replace("m=32768", "m=4294967295");
    encoded["verifier"] = serde_json::json!(verifier);
    assert!(serde_json::from_value::<AccountMaterial>(encoded).is_err());
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), before);
    assert!(vault::open_recovery(&prepared.material, &prepared.recovery).is_ok());
    assert_eq!(
        ALLOCATIONS.load(Ordering::SeqCst),
        before,
        "random recovery keys do not run Argon2"
    );
    assert!(vault::open_password(
        &prepared.material,
        &SecretText::new("incorrect-password".into())
    )
    .is_err());
    assert_eq!(UNCLEARED.load(Ordering::SeqCst), 0);
}
#[tokio::test]
async fn cancelling_the_http_waiter_does_not_release_its_running_kdf_permit() {
    let gate = Arc::new(KdfGate::new());
    let (entered, ready) = tokio::sync::oneshot::channel();
    let (release, waiting) = std::sync::mpsc::channel();
    let worker_gate = gate.clone();
    let waiter = tokio::spawn(async move {
        worker_gate
            .try_run(move || {
                let _ = entered.send(());
                waiting.recv().unwrap();
                Ok(())
            })
            .await
    });
    ready.await.unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(matches!(gate.try_run(|| Ok(())).await, Err(KdfError::Busy)));
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match gate.try_run(|| Ok(())).await {
                Ok(()) => break,
                Err(KdfError::Busy) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected KDF result: {error}"),
            }
        }
    })
    .await
    .unwrap();
}
