//! Demo data for local development (`--seed` / `DEV_SEED=1`).
//!
//! Everything goes through the real services and repositories, so the seeded
//! rows are exactly what the API would have produced. Seeding is idempotent:
//! the demo mod is the marker, and nothing is inserted when it already exists.

use std::sync::Arc;

use community_domain::mods::RawMetadata;
use community_domain::{DiscordId, User, UserId};

use crate::comments::CommentsService;
use crate::mods::ModHostingService;
use crate::ports::{Clock, PortResult, TokenGenerator, UserRepository, UserUpsert};
use crate::{AppError, DownloadService, StarService};

/// Id of the demo mod (also the idempotency marker).
pub const DEMO_MOD_ID: &str = "demo-livery";
const DEMO_VERSION: &str = "1.0.0";

/// Demo riders: `(discord id, username, display name)`. The first one authors the mod.
const DEMO_USERS: [(&str, &str, &str); 3] = [
    ("900000000000000101", "demo-author", "Demo Author"),
    ("900000000000000102", "demo-fan-one", "Fan One"),
    ("900000000000000103", "demo-fan-two", "Fan Two"),
];
const DEMO_COMMENTS: [&str; 3] = [
    "Great livery, looks amazing on the track!",
    "Works fine here. Thanks for sharing.",
    "Could you add a night variant?",
];
const DEMO_DOWNLOADS: usize = 5;

/// What a seeding run did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeedOutcome {
    Seeded,
    AlreadySeeded,
}

pub struct DevSeeder {
    pub clock: Arc<dyn Clock>,
    pub tokens: Arc<dyn TokenGenerator>,
    pub users: Arc<dyn UserRepository>,
    pub mods: Arc<ModHostingService>,
    pub comments: Arc<CommentsService>,
    pub stars: Arc<StarService>,
    pub downloads: Arc<DownloadService>,
    /// The first id signs the approval of the demo mod.
    pub admin_discord_ids: Vec<DiscordId>,
}

impl DevSeeder {
    async fn upsert(
        &self,
        discord_id: &str,
        username: &str,
        display_name: &str,
    ) -> PortResult<User> {
        let discord_id = DiscordId::parse(discord_id)
            .ok_or_else(|| AppError::Internal("the seed Discord id is invalid".into()))?;
        self.users
            .upsert(&UserUpsert {
                new_id: UserId(self.tokens.generate(16)),
                discord_id,
                username: username.to_owned(),
                global_name: Some(display_name.to_owned()),
                avatar_hash: None,
                now: self.clock.now(),
            })
            .await
    }

    pub async fn seed(&self) -> PortResult<SeedOutcome> {
        let admin_id = self.admin_discord_ids.first().ok_or_else(|| {
            AppError::Internal("seeding needs at least one admin Discord id".into())
        })?;
        let admin = self
            .upsert(admin_id.as_str(), "dev-admin", "Dev Admin")
            .await?;
        let mut riders = Vec::new();
        for (discord_id, username, display_name) in DEMO_USERS {
            riders.push(self.upsert(discord_id, username, display_name).await?);
        }
        let author = &riders[0];

        let already = self
            .mods
            .my_mods(author)
            .await?
            .iter()
            .any(|entry| entry.mod_id.as_str() == DEMO_MOD_ID);
        if already {
            return Ok(SeedOutcome::AlreadySeeded);
        }

        // One approved hosted mod with a small zip.
        let zip = demo_zip();
        let session = self
            .mods
            .create_upload(
                author,
                RawMetadata {
                    mod_id: DEMO_MOD_ID.into(),
                    title: "Demo Livery".into(),
                    description: "A demo livery seeded for local development.".into(),
                    long_description: Some(
                        "Seeded by the dev mode so the community screens are not empty.".into(),
                    ),
                    category: "liveries".into(),
                    game: "MotoGP 26".into(),
                    version: DEMO_VERSION.into(),
                    file_name: "demo-livery.zip".into(),
                    size: zip.len() as u64,
                    sha256: None,
                    ownership_confirmed: true,
                },
            )
            .await?;
        self.mods
            .write_chunk(author, &session.upload_id, 0, &zip)
            .await?;
        self.mods.complete(author, &session.upload_id).await?;
        self.mods.approve(&admin, DEMO_MOD_ID, DEMO_VERSION).await?;

        for (rider, body) in riders.iter().zip(DEMO_COMMENTS) {
            self.comments.post(rider, DEMO_MOD_ID, body).await?;
        }
        for rider in &riders {
            self.stars.star(&rider.id, DEMO_MOD_ID).await?;
        }
        for index in 0..DEMO_DOWNLOADS {
            self.downloads
                .report(DEMO_MOD_ID, &format!("dev-seed-install-{index:08}"))
                .await?;
        }
        Ok(SeedOutcome::Seeded)
    }
}

/// A valid ZIP archive with one stored file (no compression), built by hand so
/// the seed needs no archive dependency.
fn demo_zip() -> Vec<u8> {
    let name = b"README.txt";
    let data = b"Demo livery seeded by the community server dev mode.\n";
    let crc = crc32(data);
    let (size, name_len) = (data.len() as u32, name.len() as u16);

    let mut local = Vec::new();
    local.extend_from_slice(&0x0403_4b50_u32.to_le_bytes());
    local.extend_from_slice(&20_u16.to_le_bytes()); // version needed
    local.extend_from_slice(&[0; 4]); // flags, method (stored)
    local.extend_from_slice(&[0; 4]); // time, date
    local.extend_from_slice(&crc.to_le_bytes());
    local.extend_from_slice(&size.to_le_bytes()); // compressed
    local.extend_from_slice(&size.to_le_bytes()); // uncompressed
    local.extend_from_slice(&name_len.to_le_bytes());
    local.extend_from_slice(&0_u16.to_le_bytes()); // extra length
    local.extend_from_slice(name);
    local.extend_from_slice(data);

    let mut central = Vec::new();
    central.extend_from_slice(&0x0201_4b50_u32.to_le_bytes());
    central.extend_from_slice(&20_u16.to_le_bytes()); // version made by
    central.extend_from_slice(&20_u16.to_le_bytes()); // version needed
    central.extend_from_slice(&[0; 4]); // flags, method
    central.extend_from_slice(&[0; 4]); // time, date
    central.extend_from_slice(&crc.to_le_bytes());
    central.extend_from_slice(&size.to_le_bytes());
    central.extend_from_slice(&size.to_le_bytes());
    central.extend_from_slice(&name_len.to_le_bytes());
    central.extend_from_slice(&[0; 12]); // extra, comment, disk, attrs (internal)
    central.extend_from_slice(&[0; 4]); // external attributes
    central.extend_from_slice(&0_u32.to_le_bytes()); // local header offset
    central.extend_from_slice(name);

    let mut end = Vec::new();
    end.extend_from_slice(&0x0605_4b50_u32.to_le_bytes());
    end.extend_from_slice(&[0; 4]); // disk numbers
    end.extend_from_slice(&1_u16.to_le_bytes()); // entries on this disk
    end.extend_from_slice(&1_u16.to_le_bytes()); // entries total
    end.extend_from_slice(&(central.len() as u32).to_le_bytes());
    end.extend_from_slice(&(local.len() as u32).to_le_bytes());
    end.extend_from_slice(&0_u16.to_le_bytes()); // comment length

    [local, central, end].concat()
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_matches_the_standard_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn demo_zip_has_the_zip_signatures_and_a_consistent_layout() {
        let zip = demo_zip();
        assert_eq!(&zip[..4], b"PK\x03\x04");
        let end = zip.len() - 22;
        assert_eq!(&zip[end..end + 4], b"PK\x05\x06");
        let central_len = u32::from_le_bytes(zip[end + 12..end + 16].try_into().unwrap());
        let central_offset = u32::from_le_bytes(zip[end + 16..end + 20].try_into().unwrap());
        assert_eq!(
            central_offset as usize + central_len as usize,
            end,
            "the central directory ends where the end record starts"
        );
        assert_eq!(
            &zip[central_offset as usize..central_offset as usize + 4],
            b"PK\x01\x02"
        );
    }
}
