use nirvana_entity::profile;
use nirvana_entity::profile::Entity as Profile;
use sea_orm::{DatabaseConnection, EntityTrait, QueryOrder};

use crate::errors::NirvanaError;

pub(crate) async fn resolve(
    db: &DatabaseConnection,
    configured: Option<i64>,
) -> Result<Option<i64>, NirvanaError> {
    if let Some(id) = configured
        && Profile::find_by_id(id).one(db).await?.is_some()
    {
        return Ok(Some(id));
    }
    Ok(Profile::find()
        .order_by_asc(profile::COLUMN.id)
        .one(db)
        .await?
        .map(|profile| profile.id))
}
