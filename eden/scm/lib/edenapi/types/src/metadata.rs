/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This software may be used and distributed according to the terms of the
 * GNU General Public License version 2.
 */

use std::fmt;
use std::str::FromStr;

#[cfg(any(test, feature = "for-tests"))]
use quickcheck_arbitrary_derive::Arbitrary;
use serde_derive::Deserialize;
use serde_derive::Serialize;
use type_macros::auto_wire;

use crate::FileAuxData;
use crate::ServerError;

/// Directory entry metadata
#[auto_wire]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(any(test, feature = "for-tests"), derive(Arbitrary))]
pub struct DirectoryMetadata {
    // not used
}

/// File entry metadata
#[auto_wire]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(any(test, feature = "for-tests"), derive(Arbitrary))]
pub struct FileMetadata {
    // #[id(0)] # deprecated
    #[id(1)] //  deprecated, the field to be removed after 06/15/2024
    #[no_default]
    pub content_id: ContentId,
    // #[id(2)] # deprecated
    #[id(3)]
    #[no_default] // for compatibility, to be removed after 06/15/2024
    pub size: u64,
    #[id(4)]
    pub content_sha1: Sha1,
    #[id(5)] // deprecated, the field to be removed after 06/15/2024
    #[no_default]
    pub content_sha256: Sha256,
    #[id(6)]
    pub content_blake3: Blake3,
}

impl From<FileMetadata> for FileAuxData {
    fn from(val: FileMetadata) -> Self {
        FileAuxData {
            total_size: val.size,
            sha1: val.content_sha1,
            blake3: val.content_blake3,
        }
    }
}

impl From<FileAuxData> for FileMetadata {
    fn from(aux: FileAuxData) -> Self {
        Self {
            size: aux.total_size,
            content_sha1: aux.sha1,
            content_blake3: aux.blake3,
            ..Default::default()
        }
    }
}

sized_hash!(Sha1, 20);
sized_hash!(Sha256, 32);
sized_hash!(Blake3, 32);
blake2_hash!(ContentId);
blake2_hash!(FsnodeId);

#[auto_wire]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(any(test, feature = "for-tests"), derive(Arbitrary))]
pub enum FileType {
    #[id(1)]
    Regular,
    #[id(2)]
    Executable,
    #[id(3)]
    Symlink,
}

impl Default for FileType {
    fn default() -> Self {
        Self::Regular
    }
}

#[auto_wire]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[cfg_attr(any(test, feature = "for-tests"), derive(Arbitrary))]
pub enum AnyFileContentId {
    #[id(1)]
    ContentId(ContentId),
    #[id(2)]
    Sha1(Sha1),
    #[id(3)]
    Sha256(Sha256),
    #[id(4)]
    SeededBlake3(Blake3),
}

impl Default for AnyFileContentId {
    fn default() -> Self {
        AnyFileContentId::ContentId(ContentId::default())
    }
}

impl FromStr for AnyFileContentId {
    type Err = ServerError;

    fn from_str(s: &str) -> Result<AnyFileContentId, Self::Err> {
        let v: Vec<&str> = s.split('/').collect();
        if v.len() != 2 {
            return Err(Self::Err::generic(
                "AnyFileContentId parsing failure: format is 'idtype/id'",
            ));
        }
        let idtype = v[0];
        let id = v[1];
        let any_file_content_id = match idtype {
            "content_id" => AnyFileContentId::ContentId(ContentId::from_str(id)?),
            "sha1" => AnyFileContentId::Sha1(Sha1::from_str(id)?),
            "sha256" => AnyFileContentId::Sha256(Sha256::from_str(id)?),
            "seeded_blake3" => AnyFileContentId::SeededBlake3(Blake3::from_str(id)?),
            _ => {
                return Err(Self::Err::generic(
                    "AnyFileContentId parsing failure: supported id types are: 'content_id', 'sha1', 'sha256' and 'seeded_blake3'",
                ));
            }
        };
        Ok(any_file_content_id)
    }
}

impl fmt::Display for AnyFileContentId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            AnyFileContentId::ContentId(id) => write!(f, "{}", id),
            AnyFileContentId::Sha1(id) => write!(f, "{}", id),
            AnyFileContentId::Sha256(id) => write!(f, "{}", id),
            AnyFileContentId::SeededBlake3(id) => write!(f, "{}", id),
        }
    }
}
