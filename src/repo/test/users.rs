use chrono::{Months, Timelike, Utc};
use sqlx::{Pool, Postgres};
use testcontainers::clients;
use tokio::join;
use crate::dto::{Code, ExternalUser, PremiumVariant, ServiceType};
use crate::repo;
use crate::repo::services::Services;
use crate::repo::test::start_postgres;
use crate::repo::users::{UpdateTarget, UserId, Users};

const TEST_UID_EXT: i64 = 1234567890;
const TEST_NAME: &str = "kozalo";
const TEST_LOCATION: (f64, f64) = (123.45, 67.890);
const TEST_SERVICE: &str = "SadBot";

#[tokio::test]
async fn test_users() -> anyhow::Result<()> {
    let docker = clients::Cli::default();
    let (_container, db) = start_postgres(&docker).await;

    let users = repo::UsersPostgres::new(db.clone());
    let external_id = UserId::External(TEST_UID_EXT);
    let code = "ru".try_into()?;

    assert!(users.get(external_id).await?.is_none());

    let service_id = create_service(&db).await?;
    let created_user_id = create_user(&users, service_id).await?;

    test_get_user_id(&users, service_id, created_user_id).await;
    test_get_created_user(&users, external_id, created_user_id).await;
    test_update_user(&users, created_user_id, code).await?;
    test_fetch_updated_user(&users, created_user_id, code).await;

    Ok(())
}

async fn create_service(db: &Pool<Postgres>) -> anyhow::Result<i32> {
    repo::ServicesPostgres::new(db.clone())
        .create(ServiceType::TelegramBot, TEST_SERVICE)
        .await
        .map_err(Into::into)
}

async fn create_user(users: &repo::UsersPostgres, service_id: i32) -> anyhow::Result<i64> {
    let external_user = ExternalUser {
        name: Some(TEST_NAME.to_owned()),
        external_id: TEST_UID_EXT,
    };
    users.register(external_user, service_id)
        .await
        .map_err(Into::into)
}

async fn test_get_user_id(users: &repo::UsersPostgres, service_id: i32, user_id: i64) {
    let fetched_user_id = users.get_user_id(service_id, TEST_UID_EXT)
        .await
        .expect("fetched_user_id must be");
    assert_eq!(fetched_user_id, Some(user_id));
}

async fn test_get_created_user(users: &repo::UsersPostgres, external_id: UserId, created_user_id: i64) {
    let fetched_user = users.get(external_id)
        .await.expect("fetched_user must be");
    assert!(fetched_user.is_some());
    let fetched_user = fetched_user.unwrap();
    assert_eq!(fetched_user.id, created_user_id);
    assert_eq!(fetched_user.name, Some(TEST_NAME.to_owned()));
    assert!(fetched_user.language_code.is_none());
    assert!(fetched_user.location.is_none());
    assert!(fetched_user.premium_till.is_none());
}

async fn test_update_user(users: &repo::UsersPostgres, created_user_id: i64, code: Code) -> anyhow::Result<()> {
    let (r1, r2, r3) = join!(
        users.update_value(created_user_id, UpdateTarget::Language(code)),
        users.update_value(created_user_id, TEST_LOCATION.into()),
        users.activate_premium(created_user_id, PremiumVariant::Month)
    );
    (r1?, r2?);
    assert!(r3?.is_some());
    Ok(())
}

async fn test_fetch_updated_user(users: &repo::UsersPostgres, created_user_id: i64, code: Code) {
    let fetched_user = users.get(UserId::Internal(created_user_id))
        .await
        .expect("couldn't fetch the updated user")
        .expect("updated user must be");
    assert_eq!(fetched_user.language_code, Some(code));
    assert_eq!(fetched_user.location, Some(TEST_LOCATION.into()));

    let now = Utc::now()
        .checked_add_months(Months::new(1))
        .unwrap()
        .with_nanosecond(0);
    let fetched_date = fetched_user.premium_till
        .and_then(|till| till.with_nanosecond(0));
    assert_eq!(fetched_date, now);
}
