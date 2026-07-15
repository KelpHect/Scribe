use rkyv::rancor::Error;

use crate::Catalog;

pub(crate) fn decode_validated(bytes: &[u8]) -> Result<Catalog, Error> {
    rkyv::from_bytes::<Catalog, Error>(bytes)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn rejects_truncated_corrupted_and_old_schema_archives() {
        let catalog = Catalog::default();
        let archive = rkyv::to_bytes::<Error>(&catalog).unwrap();
        for length in 0..archive.len() {
            assert!(decode_validated(&archive[..length]).is_err());
        }

        let mut corrupted = archive.to_vec();
        if let Some(last) = corrupted.last_mut() {
            *last ^= 0xff;
        }
        assert!(decode_validated(&corrupted).is_err());
        assert!(decode_validated(b"SCRIBE-CATALOG-V0").is_err());
    }

    #[test]
    fn configured_unaligned_archives_are_still_validated() {
        let archive = rkyv::to_bytes::<Error>(&Catalog::default()).unwrap();
        let mut misaligned = vec![0_u8];
        misaligned.extend_from_slice(&archive);
        assert_eq!(
            decode_validated(&misaligned[1..]).unwrap(),
            Catalog::default()
        );
    }

    proptest! {
        #[test]
        fn arbitrary_archive_bytes_never_bypass_validation(bytes in prop::collection::vec(any::<u8>(), 0..65_536)) {
            let _ = decode_validated(&bytes);
        }
    }
}
