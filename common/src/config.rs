use std::{fs, net::SocketAddr, process, time::Duration};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    pub fn load(path: &str) -> Self {
        match fs::read_to_string(path) {
            Ok(data) => match toml::from_str::<Self>(&data) {
                Ok(res) => return res,
                Err(err) => eprintln!("Serialization failed: {err}"),
            },
            Err(err) => eprintln!("Readfile [{path}] failed: {err}"),
        }
        process::exit(0)
    }
}

pub fn from_duration<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserializer).map(Duration::from_millis)
}

pub fn as_duration<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_u64(duration.as_millis() as u64)
}
