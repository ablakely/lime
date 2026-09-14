use std::num::NonZeroUsize;
use std::sync::Arc;
use std::{cell::RefCell, path::Path};

use anyhow::{Context, Result, anyhow};
use lmdb_zero::{self as lmdb, LmdbResultExt};
use serde::Deserialize;

use crate::types::DatabaseFileType;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct KVKey(pub String);

pub enum KVStore {
    Lmdb {
        env: Arc<lmdb::Environment>,
        db: Arc<lmdb::Database<'static>>,
    },
    Mtbl {
        mmap: memmap::Mmap,
    },
}

#[derive(Debug, Clone)]
pub struct KVStoreCache {
    mtbl_cache: RefCell<oxidized_mtbl::BlockCache>,
}

impl KVStoreCache {
    pub fn new() -> Self {
        Self {
            mtbl_cache: RefCell::new(oxidized_mtbl::BlockCache::new(
                NonZeroUsize::new(1024).unwrap(),
            )),
        }
    }
}

impl KVStore {
    pub fn new_lmdb(path: &Path) -> Result<Self> {
        let builder = lmdb::EnvBuilder::new()?;
        let path_str = path
            .to_str()
            .ok_or_else(|| anyhow!("Unable to convert path to string"))?;
        let env = Arc::new(unsafe {
            builder
                .open(path_str, lmdb::open::RDONLY | lmdb::open::NOLOCK, 644)
                .with_context(|| {
                    format!("Failed to open LMDB database environment at \"{path_str}\"")
                })?
        });

        let db = Arc::new(
            lmdb::Database::<'static>::open(
                env.clone(),
                None,
                &lmdb::DatabaseOptions::new(lmdb::db::Flags::empty()),
            )
            .with_context(|| format!("Failed to open LMDB database at \"{path_str}\""))?,
        );

        Ok(Self::Lmdb { env, db })
    }

    pub fn new_mtbl(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mmap = Context::with_context(unsafe { memmap::Mmap::map(&file) }, || {
            format!(
                "failed to open MTBL database at \"{}\"",
                path.to_string_lossy().as_ref()
            )
        })?;

        Ok(Self::Mtbl { mmap })
    }

    pub fn new(path: &Path, db_type: DatabaseFileType) -> Result<Self> {
        match db_type {
            DatabaseFileType::Lmdb => Self::new_lmdb(path),
            DatabaseFileType::Mtbl => Self::new_mtbl(path),
        }
    }

    pub fn get(&self, key: &KVKey, cache: Option<&KVStoreCache>) -> Result<Option<Vec<u8>>> {
        match self {
            Self::Lmdb { env, db } => Ok(lmdb::ReadTransaction::new(env.clone())?
                .access()
                .get::<[u8], [u8]>(db, key.0.as_bytes())
                .to_opt()?
                .map(Vec::from)),
            Self::Mtbl { mmap } => {
                let mut reader_builder = oxidized_mtbl::ReaderBuilder::new();
                reader_builder.verify_checksums(false);
                let mut maybe_ref_mut = cache.map(|c| c.mtbl_cache.borrow_mut());
                if let Some(ref_mut) = &mut maybe_ref_mut {
                    reader_builder.block_cache(ref_mut);
                }
                Ok(reader_builder
                    .read(mmap)?
                    .get(key.0.as_bytes())?
                    .as_ref()
                    .map(|x| Vec::from(x.as_ref())))
            }
        }
    }
}
