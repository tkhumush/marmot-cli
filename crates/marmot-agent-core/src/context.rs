use mdk_core::prelude::*;
use mdk_core::key_packages::KeyPackageEventData;
use mdk_memory_storage::MdkMemoryStorage;
use nostr::{Event, EventBuilder, Keys, Kind, RelayUrl};
use tracing::info;

use crate::identity::Identity;
use crate::storage::AgentStorage;
use crate::Result;

/// Agent context wraps MDK + storage + relay connectivity.
pub struct AgentContext {
    pub mdk: MDK<MdkMemoryStorage>,
    pub storage: AgentStorage,
    pub identity: Identity,
}

impl AgentContext {
    /// Initialize with a named identity from storage.
    pub async fn with_identity(storage: AgentStorage, name: &str) -> Result<Self> {
        let identity = storage.load_identity(name).await?;
        let mdk = MDK::new(MdkMemoryStorage::default());
        Ok(Self {
            mdk,
            storage,
            identity,
        })
    }

    /// Initialize with the default identity.
    pub async fn with_default(storage: AgentStorage) -> Result<Option<Self>> {
        if let Some(identity) = storage.default_identity().await? {
            let name = identity.name.clone().unwrap_or_default();
            let mdk = MDK::new(MdkMemoryStorage::default());
            Ok(Some(Self {
                mdk,
                storage,
                identity,
            }))
        } else {
            Ok(None)
        }
    }

    /// Create a KeyPackage event (kind 30443) ready for relay publishing.
    pub fn create_keypackage(&self,
        relays: Vec<RelayUrl>,
    ) -> Result<KeyPackageEventData> {
        let data = self
            .mdk
            .create_key_package_for_event(&self.identity.keys.public_key(), relays)
            .map_err(|e| crate::Error::Identity(format!("KeyPackage creation failed: {}", e)))?;
        Ok(data)
    }

    /// Sign a KeyPackage event for publishing.
    pub fn sign_keypackage_event(&self,
        data: &KeyPackageEventData,
    ) -> Result<Event> {
        let kind = Kind::Custom(30443);
        let event = EventBuilder::new(kind, data.content.clone())
            .tags(data.tags_30443.clone())
            .sign_with_keys(&self.identity.keys)
            .map_err(|e| crate::Error::Identity(format!("event signing failed: {}", e)))?;
        Ok(event)
    }

    /// Get npub of the current identity.
    pub fn npub(&self) -> String {
        self.identity.npub()
    }

    /// Get public key hex of the current identity.
    pub fn public_key_hex(&self) -> String {
        self.identity.public_key_hex()
    }

    /// Create a new MLS group.
    pub fn create_group(
        &self,
        name: &str,
        relays: Vec<RelayUrl>,
    ) -> Result<GroupResult> {
        let config = NostrGroupConfigData::new(
            name.to_string(),
            "".to_string(), // description
            None, None, None, // image
            relays,
            vec![self.identity.keys.public_key()], // admins
            None, // disappearing messages
        );
        let result = self
            .mdk
            .create_group(
                &self.identity.keys.public_key(),
                vec![], // no initial members
                config,
            )
            .map_err(|e| crate::Error::Identity(format!("group creation failed: {}", e)))?;
        Ok(result)
    }

    /// Load all groups from storage via the group trait method.
    pub fn list_groups(&self) -> Result<Vec<group_types::Group>> {
        let groups = self
            .mdk
            .get_groups()
            .map_err(|e| crate::Error::Identity(format!("storage error: {}", e)))?;
        Ok(groups)
    }
}
