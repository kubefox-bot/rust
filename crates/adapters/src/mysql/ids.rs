use sqlx::{MySql, QueryBuilder};

pub(super) fn bind(query: &mut QueryBuilder<'_, MySql>, ids: impl Iterator<Item = i64>) {
    let mut separated = query.separated(",");
    for id in ids {
        separated.push_bind(id);
    }
}
