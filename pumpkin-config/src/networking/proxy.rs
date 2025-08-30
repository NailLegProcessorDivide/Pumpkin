use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub velocity: VelocityConfig,
    pub bungeecord: BungeeCordConfig,
}
#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct BungeeCordConfig {
    pub enabled: bool,
}

#[derive(Deserialize, Serialize, Default)]
#[serde(default)]
pub struct VelocityConfig {
    pub enabled: bool,
    pub secret: String,
}

#[derive(Deserialize, Serialize)]
pub enum ProxyType {
    BengeeCord,
    Velocity { secret: String },
}

impl ProxyConfig {
    pub fn to_proxy_type(self) -> Option<ProxyType> {
        if self.enabled {
            if self.velocity.enabled {
                Some(ProxyType::Velocity {
                    secret: self.velocity.secret,
                })
            } else if self.bungeecord.enabled {
                Some(ProxyType::BengeeCord)
            } else {
                unreachable!("proxy is enabled but no proxy is enabled, what does this even mean")
            }
        } else {
            None
        }
    }
}
