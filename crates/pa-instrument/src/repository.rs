use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct InstrumentRepository {
    pool: PgPool,
}

impl InstrumentRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::InstrumentRepository;
    use sqlx::postgres::PgPoolOptions;

    #[tokio::test]
    async fn repository_wraps_a_pg_pool() {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgres://postgres:postgres@localhost/oh_paa")
            .expect("lazy pool should be constructible without a live database");
        let repository = InstrumentRepository::new(pool.clone());

        assert_eq!(repository.pool().size(), 0);
    }
}
