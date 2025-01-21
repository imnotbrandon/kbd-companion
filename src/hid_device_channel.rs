use crate::record::Record;
use bincode::config::legacy;
use bincode::error::{DecodeError, EncodeError};
use bincode::{decode_from_slice, encode_to_vec};
use hidapi::{HidApi, HidDevice, HidError};

pub struct HidDeviceChannel {
    api: HidApi,
    device: HidDevice,
}

#[derive(Debug)]
pub enum WriteError {
    Encode(EncodeError),
    Hid(HidError),
}

pub(crate) type WriteResult = Result<usize, WriteError>;

#[derive(Debug)]
pub enum ReadError {
    Hid(HidError),
    Decode(DecodeError),
}

type ReadResult = Result<Option<Record>, ReadError>;

// return x.vendor_id() == 0x3434
// && x.product_id() == 0x0661
// && x.usage_page() == 0xFF60
// && x.usage() == 0x61;
impl HidDeviceChannel {
    pub(crate) fn connect(
        vendor_id: u16,
        product_id: u16,
        usage_page: u16,
        usage: u16,
    ) -> Result<Self, HidError> {
        let api = HidApi::new()?;
        let device = api
            .device_list()
            .find(|x| {
                return x.vendor_id() == vendor_id
                    && x.product_id() == product_id
                    && x.usage_page() == usage_page
                    && x.usage() == usage;
            })
            .ok_or_else(|| HidError::InitializationError)?
            .open_device(&api)?;

        Ok(Self { api, device })
    }
}

impl HidDeviceChannel {
    pub(crate) fn read_record(&self, timeout: Option<i32>) -> ReadResult {
        let timeout = timeout.unwrap_or(100);
        let mut data = vec![0; 32];

        let size = self
            .device
            .read_timeout(&mut data, timeout)
            .map_err(|e1| ReadError::Hid(e1))?;

        if size == 0 {
            // Timeout was reached, didn't read anything
            return Ok(None);
        }

        println!("Received Raw bytes: {:?}", data);

        decode_from_slice(&data, legacy())
            .map(|result| Some(result.0))
            .map_err(|e| ReadError::Decode(e))
    }
    pub(crate) fn write_record(&self, record: Record) -> WriteResult {
        println!("Record to write: {:?}", record);

        match encode_to_vec(record, legacy()) {
            Ok(mut encoded_data) => {
                encoded_data.resize(32, 0);
                assert_eq!(
                    encoded_data.len(),
                    32,
                    "The encoded data did not match. Expected {} got {}. Data stream: {:?}",
                    32,
                    encoded_data.len(),
                    encoded_data
                );

                println!("Data to write: {:?}", encoded_data);

                // Prepend the record ID; required
                encoded_data.insert(0, 0);

                self.device
                    .write(encoded_data.as_slice())
                    .map_err(|e| WriteError::Hid(e))
            }
            Err(err) => Err(WriteError::Encode(err)),
        }
    }
}
