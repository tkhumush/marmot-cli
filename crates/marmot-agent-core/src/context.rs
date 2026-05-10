use mdk_core::prelude::*;
use mdk_core::key_packages::KeyPackageEventData;
use mdk_sqlite_storage::MdkSqliteStorage;
use mdk_sqlite_storage::EncryptionConfig;
use nostr::{Event, EventBuilder, Kind, RelayUrl, UnsignedEvent};

use crate::identity::Identity;
use crate::storage::AgentStorage;
use crate::Result;

/// Agent context wraps MDK + storage + relay connectivity.
pub struct AgentContext {
    pub mdk: MDK<MdkSqliteStorage>,
    pub storage: AgentStorage,
    pub identity: Identity,
}

impl AgentContext {
    /// Initialize with a named identity from storage.
    pub async fn with_identity(storage: AgentStorage, name: &str) -> Result<Self> {
        let identity = storage.load_identity(name).await?;
        let db_path = storage.dirs.database_path();
        let db_key = storage.db_encryption_key().await?;
        let mdk = MDK::new(
            MdkSqliteStorage::new_with_key(
                &db_path,
                EncryptionConfig::new(db_key),
            )
            .map_err(|e| crate::Error::Storage(format!("sqlite init failed: {}", e).into()))?,
        );
        Ok(Self {
            mdk,
            storage,
            identity,
        })
    }

    /// Initialize with the default identity.
    pub async fn with_default(storage: AgentStorage) -> Result<Option<Self>> {
        if let Some(identity) = storage.default_identity().await? {
            let db_path = storage.dirs.database_path();
            let db_key = storage.db_encryption_key().await?;
            let mdk = MDK::new(
                MdkSqliteStorage::new_with_key(
                    &db_path,
                    EncryptionConfig::new(db_key),
                )
                .map_err(|e| crate::Error::Storage(format!("sqlite init failed: {}", e).into()))?,
            );
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
    pub fn create_keypackage(
        &self,
        relays: Vec<RelayUrl>,
    ) -> Result<KeyPackageEventData> {
        let data = self
            .mdk
            .create_key_package_for_event(&self.identity.keys.public_key(), relays)
            .map_err(|e| crate::Error::Identity(format!("KeyPackage creation failed: {}", e)))?;
        Ok(data)
    }

    /// Sign a KeyPackage event for publishing.
    pub fn sign_keypackage_event(&self, data: &KeyPackageEventData) -> Result<Event> {
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

    /// Create a new MLS group (e.g., for DMs or groups).
    pub fn create_group(
        &self,
        name: &str,
        relays: Vec<RelayUrl>,
    ) -> Result<GroupResult> {
        let config = NostrGroupConfigData::new(
            name.to_string(),
            "".to_string(),               // description
            None, None, None,             // image
            relays,
            vec![self.identity.keys.public_key()], // admins
            None,                         // disappearing messages
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

    /// Start a DM with someone by their KeyPackage event.
    /// Creates a 2-member MLS group. Returns the UpdateGroupResult
    /// which contains the commit_event + welcome_rumors to publish.
    pub fn create_dm(
        &self,
        name: &str,
        relays: Vec<RelayUrl>,
        member_keypackage_event: &Event,
    ) -> Result<UpdateGroupResult> {
        // 1. Create the group
        let group_result = self.create_group(name, relays)?;
        let mls_group_id = group_result.group.mls_group_id.clone();

        // 2. Parse their KeyPackage (just to verify it's valid before add_members)
        let _kp = self
            .mdk
            .parse_key_package(member_keypackage_event)
            .map_err(|e| crate::Error::Identity(format!("KeyPackage parse failed: {}", e)))?;

        // 3. Add them to the group
        let update_result = self
            .mdk
            .add_members(&mls_group_id, &[member_keypackage_event.clone()])
            .map_err(|e| {
                if e.to_string().contains("InviteeMissingRequiredProposal") {
                    crate::Error::Identity(
                        "Invitee's KeyPackage is missing required MLS proposals".to_string(),
                    )
                } else {
                    crate::Error::Identity(format!("add member failed: {}", e))
                }
            })?;

        Ok(update_result)
    }

    /// Find a group by its Nostr group ID (the 32-byte hex in the `h` tag).
    pub fn find_group_by_nostr_id(&self,
        nostr_group_id_hex: &str,
    ) -> Result<Option<group_types::Group>> {
        let target_bytes = hex::decode(nostr_group_id_hex).map_err(|e| {
            crate::Error::Identity(format!("invalid nostr group id hex: {}", e))
        })?;
        if target_bytes.len() != 32 {
            return Err(crate::Error::Identity(
                "nostr group id must be 32 bytes".to_string(),
            ));
        }
        let mut target: [u8; 32] = [0u8; 32];
        target.copy_from_slice(&target_bytes);

        let groups = self.list_groups()?;
        Ok(groups.into_iter().find(|g| g.nostr_group_id == target))
    }

    /// Build an encrypted Direct Message (MLS application message) as a kind 445 Nostr event.
    pub fn create_dm_message(
        &self,
        mls_group_id: &GroupId,
        content: &str,
    ) -> Result<Event> {
        let rumor: UnsignedEvent = EventBuilder::new(Kind::TextNote, content)
            .build(self.identity.keys.public_key());

        let event = self
            .mdk
            .create_message(mls_group_id, rumor, None)
            .map_err(|e| crate::Error::Identity(format!("DM creation failed: {}", e)))?;

        Ok(event)
    }

    /// Collect all events from an UpdateGroupResult that need publishing.
    ///
    /// Returns a Vec of (label, Event) tuples suitable for `relay::publish_events`.
    /// Labels help trace which event failed in the relay publish result.
    pub fn prepare_group_update_events(
        &self,
        result: &mdk_core::groups::UpdateGroupResult,
    ) -> Result<Vec<(&str, Event)>> {
        let mut events = Vec::with_capacity(
            1 + result.welcome_rumors.as_ref().map(|w| w.len()).unwrap_or(0),
        );

        // The evolution event is already signed
        events.push(("evolution_commit", result.evolution_event.clone()));

        // Welcome rumors need signing
        if let Some(ref rumors) = result.welcome_rumors {
            for (i, rumor) in rumors.iter().enumerate() {
                let event = rumor
                    .clone()
                    .sign_with_keys(&self.identity.keys)
                    .map_err(|e| {
                        crate::Error::Identity(format!(
                            "failed to sign welcome rumor {}: {}",
                            i, e
                        ))
                    })?;
                events.push(("welcome", event));
            }
        }

        Ok(events)
    }
}
