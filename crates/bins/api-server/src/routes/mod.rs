    Arc<dyn IdempotencyStore>,
    Option<SqlitePool>,
    Option<PgPool>,
);

impl ApiState {
    /// Compatibility constructor retained for existing test harnesses and
    /// integration tests that explicitly provide a database URL.
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let config = AppConfig::for_database(database_url)?;
        Self::new_with_config(config).await
    }

    /// Authoritative API composition entry point. All environment-sensitive
    /// startup policy is parsed and validated by AppConfig before state is built.
    pub async fn new_with_config(config: AppConfig) -> anyhow::Result<Self> {
        let postgres_primary = config.database_is_postgres();

        let (
            projection_pool,
            notification_repo,
            approval_repo,
            profile_repo,
            policy_repo,
            legal_hold_repo,
            todo_list_repo,
            target_list_repo,
            staff_loan_repo,
            unit_factory,
            idempotency_store,
            sqlite_pool,
            primary_postgres_pool,
        ): StorageBackendHandles = if postgres_primary {
            let pool = PgPoolOptions::new()
                .max_connections(20)
                .connect(config.database_url())
                .await?;
            sqlx::migrate!("../../../migrations/postgres")
                .run(&pool)
                .await?;
            (
                ProjectionPool::Postgres(pool.clone()),
                Arc::new(PostgresRepository::new(pool.clone(), "notification")),
                Arc::new(PostgresRepository::new(pool.clone(), "approval")),
                Arc::new(PostgresRepository::new(pool.clone(), "staff_profile")),
                Arc::new(PostgresRepository::new(pool.clone(), "policy")),
                Arc::new(PostgresRepository::new(pool.clone(), "legal_hold")),
                Arc::new(PostgresRepository::new(pool.clone(), "todo_list")),
                Arc::new(PostgresRepository::new(pool.clone(), "target_list")),
                Arc::new(PostgresRepository::new(pool.clone(), "staff_loan")),
                Arc::new(PostgresUnitOfWorkFactory::new(pool.clone())),
                Arc::new(PostgresIdempotencyStore::new(pool.clone())),
                None,
                Some(pool),
            )
        } else {
            let options = SqliteConnectOptions::from_str(config.database_url())?
                .create_if_missing(true)
                // Phase A fix (User Hierarchy): without this, SQLite
                // never enforces the FK on users.parent_user_id (or any
                // other FK in this schema) — confirmed by grep before
                // this fix that no connection anywhere set this pragma,
                // documented as a real gap in DECISIONS.md. Per-
                // connection, not database-wide, so it must be set here
                // on every connection the pool opens, not as a one-time
                // PRAGMA query against a single connection that would
                // be lost once the pool cycles connections.
                .foreign_keys(true);
            let pool = SqlitePoolOptions::new()
                .max_connections(if config.database_url().contains(":memory:") {
                    1
                } else {
                    5
                })
                .connect_with(options)
                .await?;
            sqlx::migrate!("../../../migrations/sqlite")
                .run(&pool)
                .await?;
            seed_if_empty(&pool).await?;
            (
                ProjectionPool::Sqlite(pool.clone()),
                Arc::new(SqliteRepository::new(pool.clone(), "notification")),
                Arc::new(SqliteRepository::new(pool.clone(), "approval")),
                Arc::new(SqliteRepository::new(pool.clone(), "staff_profile")),
                Arc::new(SqliteRepository::new(pool.clone(), "policy")),