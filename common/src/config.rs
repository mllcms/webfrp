use std::{fs, net::SocketAddr, time::Duration};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tokio::io::{self};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub secret: String,
    #[serde(serialize_with = "as_duration", deserialize_with = "from_duration")]
    pub timeout: Duration,
    #[serde(serialize_with = "as_duration", deserialize_with = "from_duration")]
    pub heartbeat: Duration,
    pub client_addr: SocketAddr,
    pub server_addr: SocketAddr,
    pub accept_addr: SocketAddr,
}

impl Config {
    pub fn load(path: &str) -> io::Result<Self> {
        let data = fs::read_to_string(path)?;
        toml::from_str(&data).map_err(io::Error::other)
    }
}

pub fn from_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(|n| Duration::from_millis(n))
}

pub fn as_duration<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u64(duration.as_millis() as u64)
}
