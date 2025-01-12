use serde::{Deserialize, Deserializer};

use std::sync::Arc;

use crate::{BlobVector, Parameter};

use super::super::*;
use super::{
    CommandToUpdate, CommandtoParam, DefBlobVector, SetBlobVector, Timestamp, UpdateError,
};

impl CommandtoParam for DefBlobVector {
    fn get_name(&self) -> &String {
        &self.name
    }
    fn get_group(&self) -> &Option<String> {
        &self.group
    }
    fn to_param(self) -> Parameter {
        Parameter::BlobVector(BlobVector {
            name: self.name,
            group: self.group,
            label: self.label,
            state: self.state,
            perm: self.perm,
            timeout: self.timeout,
            timestamp: self.timestamp.map(Timestamp::into_inner),
            values: self
                .blobs
                .into_iter()
                .map(|i| {
                    (
                        i.name,
                        crate::Blob {
                            label: i.label,
                            format: None,
                            value: None,
                        },
                    )
                })
                .collect(),
        })
    }
}

impl CommandToUpdate for SetBlobVector {
    fn get_name(&self) -> &String {
        &self.name
    }

    fn update_param(self, param: &mut Parameter) -> Result<String, UpdateError> {
        match param {
            Parameter::BlobVector(blob_vector) => {
                blob_vector.state = self.state;
                blob_vector.timeout = self.timeout;
                blob_vector.timestamp = self.timestamp.map(Timestamp::into_inner);
                for blob in self.blobs {
                    if let Some(existing) = blob_vector.values.get_mut(&blob.name) {
                        existing.format = Some(blob.format);
                        existing.value = Some(Arc::new(blob.value.into()));
                    }
                }
                Ok(self.name)
            }
            _ => Err(UpdateError::ParameterTypeMismatch(self.name.clone())),
        }
    }
}

impl From<Vec<u8>> for super::Blob {
    fn from(value: Vec<u8>) -> Self {
        super::Blob(value)
    }
}

impl From<super::Blob> for Vec<u8> {
    fn from(value: super::Blob) -> Self {
        value.0
    }
}
impl<'de> Deserialize<'de> for super::Blob {
    fn deserialize<D>(deserializer: D) -> Result<super::Blob, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        let mut result = vec![];

        for line in s.split('\n') {
            base64::decode_config_buf(line, base64::STANDARD, &mut result).unwrap();
        }

        Ok(super::Blob(result))
    }
}

impl Serialize for super::Blob {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&base64::encode(&self.0))
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        serialization::{DefBlob, EnableBlob, OneBlob},
        BlobEnable, PropertyState,
    };

    use super::*;

    #[test]
    fn test_parse_blob() {
        let xml = r#"
    <defBLOB name="INDI_DISABLED" label="Disabled"/>
"#;

        let result: DefBlob = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(
            result,
            DefBlob {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
            }
        );

        let xml = r#"
    <defBLOB name="INDI_DISABLED" label="Disabled"/>
"#;

        let result: DefBlob = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(
            result,
            DefBlob {
                name: "INDI_DISABLED".to_string(),
                label: Some("Disabled".to_string()),
            }
        );
    }

    #[test]
    fn test_send_enable_blob() {
        // let mut writer = Writer::new(Cursor::new(Vec::new()));

        let command = EnableBlob {
            device: String::from("CCD Simulator"),
            name: None,
            enabled: BlobEnable::Also,
        };

        let result = quick_xml::se::to_string(&command).unwrap();
        assert_eq!(
            result,
            String::from_str("<enableBLOB device=\"CCD Simulator\">Also</enableBLOB>").unwrap()
        );
    }

    #[test]
    fn test_set_blob() {
        let xml = include_str!("../../tests/image_capture_one_blob.log");

        let result: OneBlob = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(result.name, "CCD1".to_string());
        assert_eq!(result.size, 23040);
        assert_eq!(result.enclen, Some(30720));
        assert_eq!(result.format, ".fits");
        assert_eq!(result.value.0.len(), 23040);
    }

    #[test]
    fn test_blob_vector() {
        let xml = r#"
    <defBLOBVector device="CCD Simulator" name="SIMULATE_BAYER" label="Bayer" group="Simulator Config" perm="rw"  state="Idle" timestamp="2022-09-06T01:41:22">
    <defBLOB name="INDI_ENABLED" label="Enabled"/>
    <defBLOB name="INDI_DISABLED" label="Disabled"/>
    </defBLOBVector>
                    "#;
        let param: DefBlobVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "SIMULATE_BAYER");
        assert_eq!(param.blobs.len(), 2)
    }

    #[test]
    fn test_set_blob_vector() {
        let xml = include_str!("../../tests/image_capture_blob_vector.log");

        let param: SetBlobVector = quick_xml::de::from_str(xml).unwrap();

        assert_eq!(param.device, "CCD Simulator");
        assert_eq!(param.name, "CCD1");
        assert_eq!(param.state, PropertyState::Ok);
        assert_eq!(param.blobs.len(), 1)
    }
}
