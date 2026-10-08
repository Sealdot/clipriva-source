use std::sync::Arc;

use crate::db::Database;
use crate::local_link::LocalLinkService;

pub struct AppState {
    pub database: Arc<Database>,
    pub local_link: Arc<LocalLinkService>,
}
