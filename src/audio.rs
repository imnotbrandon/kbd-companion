use std::ops::Mul;
use windows::core::{Error, GUID};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eCommunications, EDataFlow, ERole, IMMDevice, IMMDeviceCollection,
    IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    STGM_READ,
};

#[derive(Debug)]
pub enum AudioError {
    ComInitialize(Error),
    DeviceEnumeratorError(Error),
    OpenDevice(),
    GetDevice(Error),
}

type AudioResult<T> = Result<T, AudioError>;

pub struct AudioManager {
    immdevice_enumerator: IMMDeviceEnumerator,
}

impl AudioManager {
    pub unsafe fn new() -> AudioResult<Self> {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|e2| AudioError::ComInitialize(e2))?;

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)
                .map_err(|e1| AudioError::DeviceEnumeratorError(e1))?;

        Ok(Self {
            immdevice_enumerator: enumerator,
        })
    }

    pub fn get_devices(
        &self,
        data_flow: EDataFlow,
        all_states: Option<bool>,
    ) -> Result<AudioDeviceCollection, AudioError> {
        let device_state = match all_states {
            Some(true) => DEVICE_STATE_ACTIVE,
            Some(false) => DEVICE_STATE_ACTIVE,
            None => DEVICE_STATE_ACTIVE,
        };
        unsafe {
            let device_collection = self
                .immdevice_enumerator
                .EnumAudioEndpoints(data_flow, device_state)
                .map_err(|e| AudioError::DeviceEnumeratorError(e))?;

            Ok(AudioDeviceCollection::from(device_collection))
        }
    }

    pub fn get_default_device(
        &self,
        data_flow: EDataFlow,
        role: ERole,
    ) -> AudioResult<AudioDevice<Deactivated>> {
        unsafe {
            Ok(self
                .immdevice_enumerator
                .GetDefaultAudioEndpoint(data_flow, role)
                .map_err(|e| AudioError::GetDevice(e))?
                .into())
        }
    }

    pub fn get_mic(&self) -> AudioResult<AudioDevice<Deactivated>> {
        self.get_default_device(eCapture, eCommunications)
    }
}

type AudioDeviceResult<T> = Result<T, AudioDeviceError>;
#[derive(Debug)]
pub enum AudioDeviceError {
    Activate(Error),
    Volume(Error),
    Mute(Error),
}

#[derive(Debug)]
pub struct AudioDevice<S: AudioDeviceState> {
    name: String,
    id: String,
    device: IMMDevice,
    state: S,
}

trait AudioDeviceState {}

#[derive(Debug)]
pub struct Activated {
    interface: IAudioEndpointVolume,
}

#[derive(Debug)]
pub struct Deactivated {}

impl AudioDeviceState for Activated {}
impl AudioDeviceState for Deactivated {}

impl AudioDevice<Deactivated> {
    pub fn activate(self) -> AudioDeviceResult<AudioDevice<Activated>> {
        let interface = unsafe {
            self.device
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
        }
        .map_err(|e| AudioDeviceError::Activate(e))?;

        Ok(AudioDevice::<Activated> {
            name: self.name,
            id: self.id,
            device: self.device,
            state: Activated { interface },
        })
    }
}

impl AudioDevice<Activated> {
    pub fn get_volume(&self) -> AudioDeviceResult<u8> {
        unsafe {
            Ok(self
                .state
                .interface
                .GetMasterVolumeLevelScalar()
                .map_err(|e| AudioDeviceError::Volume(e))?
                .mul(100f32)
                .round()
                .clamp(0f32, 100f32) as u8)
        }
    }

    pub fn get_muted(&self) -> AudioDeviceResult<bool> {
        unsafe {
            Ok(self
                .state
                .interface
                .GetMute()
                .map_err(|e| AudioDeviceError::Mute(e))?
                .into())
        }
    }

    pub fn set_muted(&self, muted: bool) {
        unsafe {
            self.state
                .interface
                .SetMute(muted, &GUID::new().unwrap())
                .unwrap()
        }
    }
}

impl From<IMMDevice> for AudioDevice<Deactivated> {
    fn from(value: IMMDevice) -> Self {
        let property_store = unsafe { value.OpenPropertyStore(STGM_READ) }
            .expect("Failed to open property store for audio device");

        let name = unsafe { property_store.GetValue(&PKEY_Device_FriendlyName) }
            .expect("Failed to get device name from property store");

        let id = unsafe {
            value
                .GetId()
                .unwrap()
                .to_string()
                .expect("Failed to get audio device ID")
        };

        Self {
            device: value,
            name: name.to_string(),
            id,
            state: Deactivated {},
        }
    }
}

pub struct AudioDeviceCollection {
    inner_collection: IMMDeviceCollection,
    num_devices: u32,
    current: u32,
    next: u32,
}

impl From<IMMDeviceCollection> for AudioDeviceCollection {
    fn from(value: IMMDeviceCollection) -> Self {
        let num_devices = unsafe { value.GetCount().unwrap() };
        Self {
            inner_collection: value,
            num_devices,
            current: 0,
            next: 1,
        }
    }
}

impl Iterator for AudioDeviceCollection {
    type Item = AudioDevice<Deactivated>;

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current;

        if current >= self.num_devices {
            return None;
        }

        let device = unsafe { self.inner_collection.Item(current).unwrap() };

        self.current = self.next;
        self.next = self.current + 1;

        Some(AudioDevice::from(device))
    }
}
