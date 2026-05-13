use mdk_core::prelude::*;
use mdk_core::key_packages::KeyPackageEventData;
use mdk_sqlite_storage::MdkSqliteStorage;
use mdk_sqlite_storage::EncryptionConfig;
use mdk_storage_traits::groups::Pagination;
use nostr::{Event, EventBuilder, EventId, Kind, PublicKey, RelayUrl, ToBech32, UnsignedEvent};
use nostr::nips::nip59;
use std::collections::BTreeSet;

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
        description: &str,
        relays: Vec<RelayUrl>,
    ) -> Result<GroupResult> {
        let config = NostrGroupConfigData::new(
            name.to_string(),
            description.to_string(),
            None, None, None,             // image
            relays,
            vec![self.identity.keys.public_key()], // admins
            None,                         // disappearing_messages
        );
        let result = self
            .mdk
            .create_group(
                &self.identity.keys.public_key(),
                vec![], // no initial members
                config,
            )
            .map_err(|e| crate::Error::Group(format!("group creation failed: {}", e)))?;
        Ok(result)
    }

    /// Load all groups from storage via the group trait method.
    pub fn list_groups(&self) -> Result<Vec<group_types::Group>> {
        let groups = self
            .mdk
            .get_groups()
            .map_err(|e| crate::Error::Group(format!("list groups failed: {}", e)))?;
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
        let group_result = self.create_group(name, "", relays)?;
        let mls_group_id = group_result.group.mls_group_id.clone();

        // 2. Parse their KeyPackage (just to verify it's valid before add_members)
        let _kp = self
            .mdk
            .parse_key_package(member_keypackage_event)
            .map_err(|e| crate::Error::Identity(format!("KeyPackage parse failed: {}", e)))?;

        // 3. Add them to the group. On failure, clean up the group created above.
        let update_result = self
            .mdk
            .add_members(&mls_group_id, &[member_keypackage_event.clone()])
            .map_err(|e| {
                let _ = self.mdk.delete_group(&mls_group_id);
                if e.to_string().contains("InviteeMissingRequiredProposal") {
                    crate::Error::Group(
                        "Invitee's KeyPackage is missing required MLS proposals".to_string(),
                    )
                } else {
                    crate::Error::Group(format!("add member failed: {}", e))
                }
            })?;

        Ok(update_result)
    }

    /// Find an existing 2-member DM group where the current identity and `peer` are the only members.
    /// Checks only groups with an empty name (the DM convention). Returns the first match.
    pub fn find_dm_with_peer(&self, peer: &PublicKey) -> Result<Option<group_types::Group>> {
        let our_key = self.identity.keys.public_key();
        let expected: BTreeSet<PublicKey> = [our_key, *peer].into_iter().collect();
        let groups = self.list_groups()?;
        for group in groups {
            if !group.name.is_empty() {
                continue;
            }
            if let Ok(members) = self.get_members_for_group(&group.mls_group_id) {
                if members == expected {
                    return Ok(Some(group));
                }
            }
        }
        Ok(None)
    }

    /// Find a group by its Nostr group ID (the 32-byte hex in the `h` tag).
    pub fn find_group_by_nostr_id(&self,
        nostr_group_id_hex: &str,
    ) -> Result<Option<group_types::Group>> {
        let target_bytes = hex::decode(nostr_group_id_hex).map_err(|e| {
            crate::Error::Group(format!("invalid nostr group id hex: {}", e))
        })?;
        if target_bytes.len() != 32 {
            return Err(crate::Error::Group(
                "nostr group id must be 32 bytes".to_string(),
            ));
        }
        let mut target: [u8; 32] = [0u8; 32];
        target.copy_from_slice(&target_bytes);

        let groups = self.list_groups()?;
        Ok(groups.into_iter().find(|g| g.nostr_group_id == target))
    }

    /// Build an encrypted MLS application message (kind 445). Works for DMs and groups.
    /// Pass `reply_to` to thread replies; the event ID appears as an `e` tag on the inner rumor.
    pub fn create_message(
        &self,
        mls_group_id: &GroupId,
        content: &str,
        reply_to: Option<EventId>,
    ) -> Result<Event> {
        let mut builder = EventBuilder::new(Kind::ChatMessage, content);
        if let Some(id) = reply_to {
            builder = builder.tag(nostr::Tag::event(id));
        }
        let rumor: UnsignedEvent = builder.build(self.identity.keys.public_key());
        self.mdk
            .create_message(mls_group_id, rumor, None)
            .map_err(|e| crate::Error::Group(format!("message creation failed: {}", e)))
    }

    /// Backward-compatible alias for create_message with no reply threading.
    pub fn create_dm_message(&self, mls_group_id: &GroupId, content: &str) -> Result<Event> {
        self.create_message(mls_group_id, content, None)
    }

    /// Create a kind 7 emoji reaction to a message inside an MLS group.
    pub fn create_reaction(
        &self,
        mls_group_id: &GroupId,
        target_event_id: EventId,
        emoji: &str,
    ) -> Result<Event> {
        let rumor = EventBuilder::new(Kind::Reaction, emoji)
            .tag(nostr::Tag::event(target_event_id))
            .build(self.identity.keys.public_key());
        self.mdk
            .create_message(mls_group_id, rumor, None)
            .map_err(|e| crate::Error::Group(format!("reaction creation failed: {}", e)))
    }

    /// Create a kind 5 deletion request for a message inside an MLS group.
    pub fn create_deletion(
        &self,
        mls_group_id: &GroupId,
        target_event_id: EventId,
    ) -> Result<Event> {
        let rumor = EventBuilder::new(Kind::EventDeletion, "")
            .tag(nostr::Tag::event(target_event_id))
            .build(self.identity.keys.public_key());
        self.mdk
            .create_message(mls_group_id, rumor, None)
            .map_err(|e| crate::Error::Group(format!("deletion creation failed: {}", e)))
    }

    /// Remove members from a group (admin only).
    pub fn remove_group_members(
        &self,
        mls_group_id: &GroupId,
        pubkeys: &[PublicKey],
    ) -> Result<UpdateGroupResult> {
        self.mdk
            .remove_members(mls_group_id, pubkeys)
            .map_err(|e| crate::Error::Group(format!("remove members failed: {}", e)))
    }

    /// Rename a group (admin only). Publishes a GroupContextExtensions commit.
    pub fn rename_group(&self, mls_group_id: &GroupId, name: &str) -> Result<UpdateGroupResult> {
        self.mdk
            .update_group_data(mls_group_id, NostrGroupDataUpdate::new().name(name))
            .map_err(|e| crate::Error::Group(format!("rename failed: {}", e)))
    }

    /// Promote a member to admin (admin only).
    pub fn promote_member(
        &self,
        mls_group_id: &GroupId,
        new_admin: PublicKey,
    ) -> Result<UpdateGroupResult> {
        let group = self.mdk
            .get_group(mls_group_id)
            .map_err(|e| crate::Error::Group(e.to_string()))?
            .ok_or_else(|| crate::Error::Group("group not found".to_string()))?;
        let mut admins: Vec<PublicKey> = group.admin_pubkeys.into_iter().collect();
        if !admins.contains(&new_admin) {
            admins.push(new_admin);
        }
        self.mdk
            .update_group_data(mls_group_id, NostrGroupDataUpdate::new().admins(admins))
            .map_err(|e| crate::Error::Group(format!("promote failed: {}", e)))
    }

    /// Demote an admin to member (admin only). Fails if they are the last admin.
    pub fn demote_member(
        &self,
        mls_group_id: &GroupId,
        admin: &PublicKey,
    ) -> Result<UpdateGroupResult> {
        let group = self.mdk
            .get_group(mls_group_id)
            .map_err(|e| crate::Error::Group(e.to_string()))?
            .ok_or_else(|| crate::Error::Group("group not found".to_string()))?;
        let admins: Vec<PublicKey> = group.admin_pubkeys.into_iter()
            .filter(|pk| pk != admin)
            .collect();
        if admins.is_empty() {
            return Err(crate::Error::Group("cannot remove the last admin".to_string()));
        }
        self.mdk
            .update_group_data(mls_group_id, NostrGroupDataUpdate::new().admins(admins))
            .map_err(|e| crate::Error::Group(format!("demote failed: {}", e)))
    }

    /// Remove self from the admin list. Required before leaving if you are an admin.
    pub fn self_demote(&self, mls_group_id: &GroupId) -> Result<UpdateGroupResult> {
        self.mdk
            .self_demote(mls_group_id)
            .map_err(|e| crate::Error::Group(format!("self-demote failed: {}", e)))
    }

    /// Publish a leave proposal (SelfRemove or Remove) for this group.
    /// Must not be an admin — call self_demote() first if needed.
    pub fn leave_group(&self, mls_group_id: &GroupId) -> Result<UpdateGroupResult> {
        self.mdk
            .leave_group(mls_group_id)
            .map_err(|e| crate::Error::Group(format!("leave group failed: {}", e)))
    }

    /// Decline a specific pending welcome invitation by its nostr_group_id hex.
    pub fn decline_welcome_by_nostr_id(&self, nostr_group_id_hex: &str) -> Result<()> {
        let welcomes = self.list_pending_welcomes()?;
        let welcome = welcomes.into_iter()
            .find(|w| hex::encode(w.nostr_group_id) == nostr_group_id_hex)
            .ok_or_else(|| crate::Error::Group(
                format!("no pending invitation for group {}", nostr_group_id_hex)
            ))?;
        self.mdk
            .decline_welcome(&welcome)
            .map_err(|e| crate::Error::Group(format!("decline failed: {}", e)))
    }

    /// Accept a single specific pending welcome invitation by its nostr_group_id hex.
    pub fn accept_welcome_by_nostr_id(&self, nostr_group_id_hex: &str) -> Result<()> {
        let welcomes = self.list_pending_welcomes()?;
        let welcome = welcomes.into_iter()
            .find(|w| hex::encode(w.nostr_group_id) == nostr_group_id_hex)
            .ok_or_else(|| crate::Error::Group(
                format!("no pending invitation for group {}", nostr_group_id_hex)
            ))?;
        self.accept_welcome(&welcome)
    }

    /// Return the evolution commit event from an UpdateGroupResult, ready to publish.
    pub fn evolution_event(result: &mdk_core::groups::UpdateGroupResult) -> &Event {
        &result.evolution_event
    }

    /// NIP-59 gift-wrap a single welcome rumor for a recipient.
    /// The resulting kind-1059 event is what gets published to relays.
    pub async fn gift_wrap_welcome(
        &self,
        rumor: UnsignedEvent,
        recipient: &PublicKey,
    ) -> Result<Event> {
        // NIP-40: ~30-day expiration matches White Noise's gift-wrap format.
        let expiry = nostr::Timestamp::from_secs(nostr::Timestamp::now().as_secs() + 30 * 24 * 3600);
        nostr::event::builder::EventBuilder::gift_wrap(
            &self.identity.keys,
            recipient,
            rumor,
            [nostr::Tag::expiration(expiry)],
        )
        .await
        .map_err(|e| crate::Error::Group(format!("gift_wrap failed: {}", e)))
    }

    /// Process an incoming encrypted Nostr event (kind 445 / commit / proposal).
    /// Decrypts application messages and stores them; advances MLS epoch on commits.
    pub fn process_incoming_event(&self, event: &Event) -> Result<MessageProcessingResult> {
        self.mdk
            .process_message(event)
            .map_err(|e| crate::Error::Group(format!("failed to process event: {}", e)))
    }

    /// Retrieve stored decrypted messages for a group, newest first.
    pub fn get_messages_for_group(
        &self,
        mls_group_id: &GroupId,
        limit: usize,
    ) -> Result<Vec<message_types::Message>> {
        self.mdk
            .get_messages(mls_group_id, Some(Pagination::new(Some(limit), Some(0))))
            .map_err(|e| crate::Error::Group(format!("failed to get messages: {}", e)))
    }

    /// Return the set of member public keys for a group.
    pub fn get_members_for_group(
        &self,
        mls_group_id: &GroupId,
    ) -> Result<BTreeSet<PublicKey>> {
        self.mdk
            .get_members(mls_group_id)
            .map_err(|e| crate::Error::Group(format!("failed to get members: {}", e)))
    }

    /// Add multiple members to an existing group in one MLS commit.
    /// Returns the UpdateGroupResult with one welcome rumor per new member (same order).
    pub fn invite_members_to_group(
        &self,
        mls_group_id: &GroupId,
        member_keypackage_events: &[Event],
    ) -> Result<UpdateGroupResult> {
        self.mdk
            .add_members(mls_group_id, member_keypackage_events)
            .map_err(|e| {
                if e.to_string().contains("InviteeMissingRequiredProposal") {
                    crate::Error::Group(
                        "A member's KeyPackage is missing required MLS proposals".to_string(),
                    )
                } else {
                    crate::Error::Group(format!("add members failed: {}", e))
                }
            })
    }

    /// Add a member to an existing group (invite flow — admin side).
    /// Returns the UpdateGroupResult with events to publish.
    pub fn invite_member_to_group(
        &self,
        mls_group_id: &GroupId,
        member_keypackage_event: &Event,
    ) -> Result<UpdateGroupResult> {
        self.mdk
            .add_members(mls_group_id, &[member_keypackage_event.clone()])
            .map_err(|e| {
                if e.to_string().contains("InviteeMissingRequiredProposal") {
                    crate::Error::Group(
                        "Invitee's KeyPackage is missing required MLS proposals".to_string(),
                    )
                } else {
                    crate::Error::Group(format!("add member failed: {}", e))
                }
            })
    }

    /// Delete a group from local storage (removes all associated MLS state).
    pub fn delete_group(&self, mls_group_id: &GroupId) -> Result<()> {
        self.mdk
            .delete_group(mls_group_id)
            .map_err(|e| crate::Error::Group(format!("failed to delete group: {}", e)))
    }

    /// Return the relay URLs configured for a group (stored in MLS group state).
    /// Use these when publishing events for the group so the recipient's subscription receives them.
    pub fn get_group_relays(&self, mls_group_id: &GroupId) -> Result<Vec<String>> {
        self.mdk
            .get_relays(mls_group_id)
            .map(|set| set.into_iter().map(|url| url.to_string()).collect())
            .map_err(|e| crate::Error::Group(format!("failed to get group relays: {}", e)))
    }

    /// Return the nostr_group_id (h-tag hex) for display.
    pub fn nostr_group_id_hex(group: &group_types::Group) -> String {
        hex::encode(group.nostr_group_id)
    }

    /// Format a member PublicKey as npub for display.
    pub fn member_npub(pk: &PublicKey) -> String {
        pk.to_bech32().expect("valid public key always encodes to bech32")
    }

    /// Unwrap a NIP-59 gift-wrap event and store it as a pending welcome if it carries a
    /// kind 444 (MlsWelcome) rumor. Returns `None` for non-welcome gift wraps.
    pub async fn unwrap_and_process_welcome(
        &self,
        gift_wrap: &Event,
    ) -> Result<Option<welcome_types::Welcome>> {
        let unwrapped = nip59::extract_rumor(&self.identity.keys, gift_wrap)
            .await
            .map_err(|e| crate::Error::Group(format!("NIP-59 unwrap failed: {}", e)))?;

        if unwrapped.rumor.kind != Kind::MlsWelcome {
            return Ok(None);
        }

        let welcome = self
            .mdk
            .process_welcome(&gift_wrap.id, &unwrapped.rumor)
            .map_err(|e| crate::Error::Group(format!("process_welcome failed: {}", e)))?;

        Ok(Some(welcome))
    }

    /// List all pending welcomes stored locally (kind 444 events not yet accepted/declined).
    pub fn list_pending_welcomes(&self) -> Result<Vec<welcome_types::Welcome>> {
        self.mdk
            .get_pending_welcomes(None)
            .map_err(|e| crate::Error::Group(format!("get_pending_welcomes failed: {}", e)))
    }

    /// Accept a pending welcome, joining the associated MLS group.
    pub fn accept_welcome(&self, welcome: &welcome_types::Welcome) -> Result<()> {
        self.mdk
            .accept_welcome(welcome)
            .map_err(|e| crate::Error::Group(format!("accept_welcome failed: {}", e)))
    }

    /// Return the IDs of groups that need a self-update commit (key rotation).
    /// Pass `threshold_secs = 0` to get all groups needing an update.
    pub fn groups_needing_self_update(&self, threshold_secs: u64) -> Result<Vec<GroupId>> {
        self.mdk
            .groups_needing_self_update(threshold_secs)
            .map_err(|e| crate::Error::Group(format!("groups_needing_self_update failed: {}", e)))
    }

    /// Perform a self-update commit for a group (rotates our leaf key).
    /// Returns the UpdateGroupResult containing the evolution event to publish.
    pub fn self_update_group(&self, mls_group_id: &GroupId) -> Result<UpdateGroupResult> {
        self.mdk
            .self_update(mls_group_id)
            .map_err(|e| crate::Error::Group(format!("self_update failed: {}", e)))
    }
}
