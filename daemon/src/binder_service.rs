use crate::GameKeyCompound;
use async_trait::async_trait;
use gamekeyd_aidl::{
    aidl::org::ingres::gamekeys::{
        ISettingsService::{self, ISettingsServiceAsyncServer, ISettingsServiceDefaultRef},
        Point::Point,
    },
    binder::{Interface, Result},
};
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct SettingsService(Arc<RwLock<GameKeyCompound>>);

impl Interface for SettingsService {}

#[allow(non_snake_case)]
#[async_trait]
impl ISettingsServiceAsyncServer for SettingsService {
    async fn r#setSettings<'a, 'l1, 'l2>(
        &'a self,
        upper: Option<&'l1 Point>,
        lower: Option<&'l2 Point>,
    ) -> Result<()> {
        let mut compound = self.0.write().await;
        compound.upper = upper.map(|point| (point.x * 10, point.y * 10));
        compound.lower = lower.map(|point| (point.x * 10, point.y * 10));

        Ok(())
    }
}

impl SettingsService {
    pub fn new(compound: Arc<RwLock<GameKeyCompound>>) -> Self {
        Self { 0: compound }
    }
}
