use serde::Deserialize;
use utoipa::IntoParams;

const fn _page_default() -> usize {
    1
}
const fn _limit_default() -> usize {
    20
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct Pagination {
    #[serde(default = "_page_default")]
    pub page: usize,
    #[serde(default = "_limit_default")]
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
