use btleplug::api::{Characteristic, Peripheral as PeripheralTrait};
use btleplug::platform::Peripheral;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DeviceConfigError {
    #[error("Data too short")]
    DataTooShort,
    #[error("BLE error: {0}")]
    BleError(#[from] btleplug::Error),
}

#[derive(Clone, Copy, Debug)]
pub struct DildonicaZoneConfig {
    pub enabled: bool,
    pub midi_control: u8,
    pub cycle_count_begin: u32,
    pub cycle_count_end: u32,
    pub comp_thresh_lo: u32,
    pub comp_thresh_hi: u32,
}

impl Default for DildonicaZoneConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            midi_control: 0,
            cycle_count_begin: 1000,
            cycle_count_end: 10000,
            comp_thresh_lo: 100,
            comp_thresh_hi: 4000,
        }
    }
}

impl DildonicaZoneConfig {
    pub const SIZE: usize = 20; // 1 + 1 + 2 (padding) + 4 + 4 + 4 + 4 = 20 bytes (4-byte aligned)

    pub fn to_bytes(&self) -> [u8; Self::SIZE] {
        let mut bytes = [0u8; Self::SIZE];
        bytes[0] = self.enabled as u8;
        bytes[1] = self.midi_control;
        // bytes[2..4] are padding for 4-byte alignment
        bytes[4..8].copy_from_slice(&self.cycle_count_begin.to_le_bytes());
        bytes[8..12].copy_from_slice(&self.cycle_count_end.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.comp_thresh_lo.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.comp_thresh_hi.to_le_bytes());
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, DeviceConfigError> {
        if bytes.len() < Self::SIZE {
            return Err(DeviceConfigError::DataTooShort);
        }

        Ok(Self {
            enabled: bytes[0] != 0,
            midi_control: bytes[1],
            // Skip bytes[2..4] (padding)
            cycle_count_begin: u32::from_le_bytes(bytes[4..8].try_into().unwrap()),
            cycle_count_end: u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
            comp_thresh_lo: u32::from_le_bytes(bytes[12..16].try_into().unwrap()),
            comp_thresh_hi: u32::from_le_bytes(bytes[16..20].try_into().unwrap()),
        })
    }
}

pub async fn read_zone_configs(
    device: &Peripheral,
    config_char: &Characteristic,
    num_zones: usize,
) -> Result<Vec<DildonicaZoneConfig>, DeviceConfigError> {
    let data = device.read(config_char).await?;
    let expected_size = DildonicaZoneConfig::SIZE * num_zones;

    if data.len() != expected_size {
        return Err(DeviceConfigError::DataTooShort);
    }

    let mut configs = Vec::with_capacity(num_zones);
    for i in 0..num_zones {
        let start = i * DildonicaZoneConfig::SIZE;
        let end = start + DildonicaZoneConfig::SIZE;
        configs.push(DildonicaZoneConfig::from_bytes(&data[start..end])?);
    }

    Ok(configs)
}

pub async fn write_zone_configs(
    device: &Peripheral,
    config_char: &Characteristic,
    configs: &[DildonicaZoneConfig],
) -> Result<(), DeviceConfigError> {
    let mut data = Vec::with_capacity(DildonicaZoneConfig::SIZE * configs.len());
    for config in configs {
        data.extend_from_slice(&config.to_bytes());
    }

    device
        .write(config_char, &data, btleplug::api::WriteType::WithResponse)
        .await?;
    Ok(())
}