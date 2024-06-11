use std::{collections::HashMap, fmt};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Nic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp4: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nameservers: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

impl fmt::Display for Nic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(s) = serde_yaml::to_string(self) {
            write!(f, "{s}")
        } else {
            Ok(())
        }
    }
}

impl Nic {
    #[must_use]
    pub fn new(
        addresses: Option<Vec<String>>,
        dhcp4: Option<bool>,
        gateway4: Option<String>,
        nameservers: Option<HashMap<String, Vec<String>>>,
        optional: Option<bool>,
    ) -> Self {
        Nic {
            addresses,
            dhcp4,
            gateway4,
            nameservers,
            optional,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NicOutput {
    pub addresses: Option<Vec<String>>,
    pub dhcp4: Option<bool>,
    pub gateway4: Option<String>,
    pub nameservers: Option<Vec<String>>,
}

impl fmt::Display for NicOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(v) = &self.addresses {
            writeln!(f, "\taddresses: {v:?}")?;
        } else {
            writeln!(f, "\taddresses: -")?;
        }
        if let Some(v) = self.dhcp4 {
            writeln!(f, "\tdhcp4: {v}")?;
        } else {
            writeln!(f, "\tdhcp4: -")?;
        }
        if let Some(v) = &self.gateway4 {
            writeln!(f, "\tgateway4: {v}")?;
        } else {
            writeln!(f, "\tgateway4: -")?;
        }
        if let Some(v) = &self.nameservers {
            write!(f, "\tnameservers: {v:?}")
        } else {
            write!(f, "\tnameservers: -")
        }
    }
}

impl NicOutput {
    #[must_use]
    pub fn new(
        addresses: Option<Vec<String>>,
        dhcp4: Option<bool>,
        gateway4: Option<String>,
        nameservers: Option<Vec<String>>,
    ) -> Self {
        NicOutput {
            addresses,
            dhcp4,
            gateway4,
            nameservers,
        }
    }

    #[must_use]
    pub fn to(&self) -> Nic {
        let nameservers = if let Some(nm) = &self.nameservers {
            let mut m = HashMap::new();
            m.insert("addresses".to_string(), nm.clone());
            m.insert("search".to_string(), Vec::new());
            Some(m)
        } else {
            None
        };
        Nic {
            addresses: self.addresses.clone(),
            dhcp4: self.dhcp4,
            gateway4: self.gateway4.clone(),
            nameservers,
            optional: None,
        }
    }

    #[must_use]
    pub fn from(nic: &Nic) -> Self {
        let nameservers = {
            if let Some(nm) = &nic.nameservers {
                nm.get("addresses").cloned()
            } else {
                None
            }
        };
        NicOutput {
            addresses: nic.addresses.clone(),
            dhcp4: nic.dhcp4,
            gateway4: nic.gateway4.clone(),
            nameservers,
        }
    }
}
