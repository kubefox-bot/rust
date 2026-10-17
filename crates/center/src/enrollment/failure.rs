use super::{remote::Connector, EnrollmentService};
impl<C: Connector> EnrollmentService<C> {
    pub(super) async fn failed(
        &self,
        view: &mut hmail_cleaner_protocol::browser::enrollment::EnrollmentView,
    ) {
        use hmail_cleaner_protocol::browser::enrollment::EnrollmentPhase;
        if view.phase == EnrollmentPhase::Ready {
            return;
        }
        let release = matches!(
            view.phase,
            EnrollmentPhase::Preparing
                | EnrollmentPhase::Transferring
                | EnrollmentPhase::IdentityIssued
        );
        let phase = if release {
            EnrollmentPhase::Failed
        } else {
            EnrollmentPhase::Unknown
        };
        if self.record(view, phase).await.is_err() {
            eprintln!("Enrollment checkpoint could not be persisted; reservation retained");
        }
    }
}
