use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZoneMapError {
    #[error("Invalid zone map: {0}")]
    InvalidZoneMap(String),
}

pub fn parse_zone_map(map_str: &str, num_zones: usize) -> Result<Vec<usize>, ZoneMapError> {
    let parts: Vec<&str> = map_str.split(',').collect();
    if parts.len() != num_zones {
        return Err(ZoneMapError::InvalidZoneMap(format!(
            "Expected {} zones, got {}",
            num_zones,
            parts.len()
        )));
    }

    let mut zone_map = Vec::with_capacity(num_zones);
    let mut used_zones = vec![false; num_zones];

    for (_i, part) in parts.iter().enumerate() {
        let zone: usize = part
            .trim()
            .parse()
            .map_err(|_| ZoneMapError::InvalidZoneMap(format!("Invalid zone number: '{}'", part)))?;

        if zone >= num_zones {
            return Err(ZoneMapError::InvalidZoneMap(format!(
                "Zone {} is out of range (0-{})",
                zone,
                num_zones - 1
            )));
        }

        if used_zones[zone] {
            return Err(ZoneMapError::InvalidZoneMap(format!(
                "Zone {} is used multiple times",
                zone
            )));
        }

        zone_map.push(zone);
        used_zones[zone] = true;
    }

    Ok(zone_map)
}

pub fn create_default_zone_map(num_zones: usize) -> Vec<usize> {
    (0..num_zones).collect()
}