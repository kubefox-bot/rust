use super::{remote::Connector, EnrollmentError, EnrollmentService};
use crate::auth::SessionLease;
use hmail_cleaner_protocol::browser::enrollment::{EnrollmentPhase, EnrollmentView};
impl<C: Connector> EnrollmentService<C> {
    pub async fn reconcile(
        &self,
        id: uuid::Uuid,
        lease: &SessionLease,
    ) -> Result<EnrollmentView, EnrollmentError> {
        let service = self.clone();
        let lease = lease.clone();
        tokio::spawn(async move {
            let mut view = service.get(id, &lease).await?;
            if view.slot_held
                && matches!(
                    view.phase,
                    EnrollmentPhase::Installing
                        | EnrollmentPhase::Verifying
                        | EnrollmentPhase::Unknown
                )
            {
                service.verify(&mut view, &lease).await?;
            }
            service.load(id).await
        })
        .await
        .map_err(|_| EnrollmentError::Storage)?
    }
}
