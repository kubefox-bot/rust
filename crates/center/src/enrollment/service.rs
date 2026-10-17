use super::{remote::Connector, NativeSsh, PackageCatalog};
use crate::storage::CenterStore;
use std::sync::Arc;
const MAX_SSH_PROBES: usize = 2;
pub struct EnrollmentService<C = NativeSsh> {
    pub(super) store: CenterStore,
    pub(super) connector: Arc<C>,
    pub(super) catalog: PackageCatalog,
    pub(super) probes: Arc<tokio::sync::Semaphore>,
}
impl<C> Clone for EnrollmentService<C> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
            connector: self.connector.clone(),
            catalog: self.catalog.clone(),
            probes: self.probes.clone(),
        }
    }
}
impl<C: Connector> EnrollmentService<C> {
    pub fn new(store: CenterStore, catalog: PackageCatalog, connector: C) -> Self {
        Self {
            store,
            catalog,
            connector: Arc::new(connector),
            probes: Arc::new(tokio::sync::Semaphore::new(MAX_SSH_PROBES)),
        }
    }
}
