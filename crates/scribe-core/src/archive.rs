use rkyv::rancor::Error;
use self_cell::self_cell;

use crate::{Catalog, Category, RemoteAddon};

struct CatalogRef<'a>(&'a rkyv::Archived<Catalog>);

self_cell!(
    pub struct CatalogArchive {
        owner: rkyv::util::AlignedVec<16>,

        #[covariant]
        dependent: CatalogRef,
    }
);

impl CatalogArchive {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        let mut aligned = rkyv::util::AlignedVec::<16>::with_capacity(bytes.len());
        aligned.extend_from_slice(bytes);
        Self::try_new(aligned, |bytes| {
            rkyv::access::<rkyv::Archived<Catalog>, Error>(bytes).map(CatalogRef)
        })
    }

    pub fn byte_len(&self) -> usize {
        self.borrow_owner().len()
    }

    pub fn len(&self) -> usize {
        self.borrow_dependent().0.addons.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn with_catalog<R>(&self, operation: impl FnOnce(&rkyv::Archived<Catalog>) -> R) -> R {
        operation(self.borrow_dependent().0)
    }

    pub fn addon_owned(&self, index: usize) -> Result<Option<RemoteAddon>, Error> {
        self.with_catalog(|catalog| {
            catalog
                .addons
                .get(index)
                .map(rkyv::deserialize::<RemoteAddon, Error>)
                .transpose()
        })
    }

    pub fn categories_owned(&self) -> Result<Vec<Category>, Error> {
        self.with_catalog(|catalog| rkyv::deserialize::<Vec<Category>, Error>(&catalog.categories))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_a_validated_archive_and_materializes_only_requested_rows() {
        let catalog = Catalog {
            addons: vec![RemoteAddon {
                uid: "42".into(),
                ui_name: "Archive row".into(),
                ..RemoteAddon::default()
            }],
            categories: Vec::new(),
        };
        let bytes = rkyv::to_bytes::<Error>(&catalog).unwrap();
        let archive = CatalogArchive::from_bytes(&bytes).unwrap();

        assert_eq!(archive.len(), 1);
        assert_eq!(archive.addon_owned(0).unwrap().unwrap().uid, "42");
        assert!(CatalogArchive::from_bytes(&bytes[..bytes.len() / 2]).is_err());
    }
}
