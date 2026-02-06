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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nic_output_to_from_roundtrip() {
        let output = NicOutput::new(
            Some(vec!["10.0.0.1/24".to_string(), "10.0.0.2/24".to_string()]),
            Some(true),
            Some("10.0.0.254".to_string()),
            Some(vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()]),
        );

        let nic = output.to();
        assert_eq!(
            nic.addresses,
            Some(vec!["10.0.0.1/24".to_string(), "10.0.0.2/24".to_string()])
        );
        assert_eq!(nic.dhcp4, Some(true));
        assert_eq!(nic.gateway4, Some("10.0.0.254".to_string()));
        assert_eq!(nic.optional, None);

        let nameservers = nic.nameservers.as_ref().expect("nameservers should be set");
        assert_eq!(
            nameservers.get("addresses"),
            Some(&vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()])
        );
        assert_eq!(nameservers.get("search"), Some(&Vec::new()));

        let roundtrip = NicOutput::from(&nic);
        assert_eq!(roundtrip.addresses, output.addresses);
        assert_eq!(roundtrip.dhcp4, output.dhcp4);
        assert_eq!(roundtrip.gateway4, output.gateway4);
        assert_eq!(roundtrip.nameservers, output.nameservers);
    }

    #[test]
    fn test_nic_output_display_full() {
        let output = NicOutput::new(
            Some(vec!["10.0.0.1/24".to_string()]),
            Some(true),
            Some("10.0.0.254".to_string()),
            Some(vec!["8.8.8.8".to_string()]),
        );

        let rendered = output.to_string();
        assert_eq!(
            rendered,
            "\taddresses: [\"10.0.0.1/24\"]\n\tdhcp4: true\n\tgateway4: 10.0.0.254\n\tnameservers: [\"8.8.8.8\"]"
        );
    }

    #[test]
    fn test_nic_output_display_empty() {
        let output = NicOutput::new(None, None, None, None);
        let rendered = output.to_string();
        assert_eq!(
            rendered,
            "\taddresses: -\n\tdhcp4: -\n\tgateway4: -\n\tnameservers: -"
        );
    }

    #[test]
    fn test_nic_output_to_without_nameservers() {
        let output = NicOutput::new(
            Some(vec!["10.0.0.1/24".to_string()]),
            Some(false),
            Some("10.0.0.254".to_string()),
            None,
        );
        let nic = output.to();

        assert_eq!(nic.addresses, output.addresses);
        assert_eq!(nic.dhcp4, output.dhcp4);
        assert_eq!(nic.gateway4, output.gateway4);
        assert_eq!(nic.nameservers, None);
        assert_eq!(nic.optional, None);
    }

    #[test]
    fn test_nic_output_from_missing_nameserver_addresses() {
        let mut nameservers = HashMap::new();
        nameservers.insert("search".to_string(), vec!["example.local".to_string()]);

        let nic = Nic::new(None, None, None, Some(nameservers), None);
        let output = NicOutput::from(&nic);

        assert_eq!(output.nameservers, None);
    }

    #[test]
    fn test_nic_output_from_without_nameservers() {
        let nic = Nic::new(
            Some(vec!["10.0.0.1/24".to_string()]),
            Some(true),
            Some("10.0.0.254".to_string()),
            None,
            None,
        );
        let output = NicOutput::from(&nic);

        assert_eq!(output.addresses, nic.addresses);
        assert_eq!(output.dhcp4, nic.dhcp4);
        assert_eq!(output.gateway4, nic.gateway4);
        assert_eq!(output.nameservers, None);
    }

    #[test]
    fn test_nic_display_includes_fields() {
        let nic = Nic::new(
            Some(vec!["10.0.0.1/24".to_string()]),
            Some(true),
            None,
            None,
            None,
        );
        let rendered = nic.to_string();
        assert!(rendered.contains("addresses:"));
        assert!(rendered.contains("- 10.0.0.1/24"));
        assert!(rendered.contains("dhcp4: true"));
    }
}
