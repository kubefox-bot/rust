pub(super) mod deletion;
pub(super) mod selection;
use hmail_cleaner_core::maintenance::OrphanTable;

pub(super) struct Table {
    pub name: &'static str,
    pub id: &'static str,
    pub parent: &'static str,
}
pub(super) fn descriptor(table: OrphanTable) -> Table {
    match table {
        OrphanTable::Metadata => Table {
            name: "hm_message_metadata",
            id: "metadata_id",
            parent: "metadata_messageid",
        },
        OrphanTable::Recipients => Table {
            name: "hm_messagerecipients",
            id: "recipientid",
            parent: "recipientmessageid",
        },
    }
}
