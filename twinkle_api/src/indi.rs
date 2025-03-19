pub mod api {
    use serde::{Deserialize, Serialize};

    use crate::{analysis::Statistics, fits::FitsImage};

    #[derive(Serialize, Deserialize)]
    pub struct ImageResponse<'a> {
        pub stats: Statistics,
        #[serde(borrow)]
        pub image: FitsImage<'a>,
    }

    

    impl<'a> ImageResponse<'a> {
        pub fn from_bytes(bytes: &'a [u8]) -> Result<ImageResponse<'a>, rmp_serde::decode::Error> {
            rmp_serde::from_slice(bytes)
        }
    
        pub fn to_bytes(&self, bytes: &mut Vec<u8>) {
            let mut serializer =
                rmp_serde::Serializer::new(bytes).with_bytes(rmp_serde::config::BytesMode::ForceAll);
        
            self.serialize(&mut serializer).unwrap();
        }
    }
}
