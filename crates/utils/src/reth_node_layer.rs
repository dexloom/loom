pub struct RethDbLayer {
    db_path: PathBuf,
}

impl RethDbLayer {
    pub(crate) const fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub(crate) const fn db_path(&self) -> &PathBuf {
        &self.db_path
    }
}