use serde::Deserialize;
use utoipa::IntoParams;

#[derive(Debug, Deserialize, IntoParams)]
pub struct Pagination {
    pub page: usize,
    pub limit: usize,
}

impl Pagination {
    pub fn start(&self) -> usize {
        if self.page == 0 {
            return 0;
        }
        (self.page - 1) * self.limit
    }
}
