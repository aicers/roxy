use std::{
    collections::HashMap,
    fmt,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::IpAddr,
    process::Command,
};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Local};
use ipnet::IpNet;
use pnet::datalink::interfaces;
use serde_derive::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Nic, NicOutput};

const NETPLAN_PATH: &str = "/etc/netplan";
const DEFAULT_NETPLAN_YAML: &str = "01-netcfg.yaml";

#[derive(Debug, Deserialize, Serialize)]
struct Address {
    #[serde(skip_serializing_if = "Option::is_none")]
    search: Option<Vec<String>>,
    addresses: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Bridge {
    interfaces: Vec<String>,
    addresses: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gateway4: Option<String>,
    nameservers: Address,
}

// only support ethernets, bridges. No wifis support.
#[serde_as]
#[derive(Debug, Deserialize, Serialize)]
struct Network {
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    renderer: Option<String>,
    #[serde_as(as = "HashMap<_, _>")]
    ethernets: Vec<(String, Nic)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bridges: Option<HashMap<String, Bridge>>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NetplanYaml {
    network: Network,
}

impl fmt::Display for NetplanYaml {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(s) = serde_yaml::to_string(self) {
            write!(f, "{s}")
        } else {
            Ok(())
        }
    }
}

impl NetplanYaml {
    fn new(path: &str) -> Result<Self> {
        let mut f = File::open(path)?;
        let mut buf = String::new();
        f.read_to_string(&mut buf)?;
        match serde_yaml::from_str::<NetplanYaml>(&buf) {
            Ok(r) => Ok(r),
            Err(e) => Err(anyhow!("Error: {e}")),
        }
    }

    // Merges two yaml conf into one. The merged conf will applied to system when save() is called.
    fn merge(&mut self, newyml: Self) {
        if newyml.network.version.is_some() {
            self.network.version = newyml.network.version;
        }
        if newyml.network.renderer.is_some() {
            self.network.renderer = newyml.network.renderer;
        }
        for (ifname, ifcfg) in newyml.network.ethernets {
            if let Some(item) = self.network.ethernets.iter_mut().find(|x| x.0 == ifname) {
                item.1 = ifcfg;
            } else {
                self.network.ethernets.push((ifname, ifcfg));
            }
        }
        self.network.ethernets.sort_by(|a, b| a.0.cmp(&b.0));

        if let Some(new_bridges) = newyml.network.bridges
            && let Some(self_bridges) = &mut self.network.bridges
        {
            for (ifname, bridgecfg) in new_bridges {
                if let Some(item) = self_bridges.get_mut(&ifname) {
                    *item = bridgecfg;
                } else {
                    self_bridges.insert(ifname, bridgecfg);
                }
            }
        }
    }

    // apply() should be run to apply this change.
    fn set_interface(&mut self, ifname: &str, new_if: Nic) {
        if let Some(item) = self.network.ethernets.iter_mut().find(|x| x.0 == *ifname) {
            item.1 = new_if;
        } else {
            self.network.ethernets.push((ifname.to_string(), new_if));
            self.network.ethernets.sort_by(|a, b| a.0.cmp(&b.0));
        }
    }

    // apply() should be run to apply this change.
    fn init_interface(&mut self, ifname: &str) {
        let new_if = Nic::new(None, None, None, None, None);
        Self::set_interface(self, ifname, new_if);
    }

    // Removes interface address, gateway4, nameservers. apply() should be run to apply this change.
    // Use set() command instead of delete() if possible
    fn delete(&mut self, ifname: &str, nic_output: &NicOutput) -> Result<()> {
        let (_, ifs) = self
            .network
            .ethernets
            .iter_mut()
            .find(|x| x.0 == *ifname)
            .ok_or_else(|| anyhow!("Interface {ifname} not found"))?;
        if let Some(addrs) = &nic_output.addresses {
            for addr in addrs {
                if let Some(ifs_addrs) = &mut ifs.addresses {
                    ifs_addrs.retain(|x| *x != *addr);
                }
            }
        }

        if nic_output.gateway4.is_some() && ifs.gateway4 == nic_output.gateway4 {
            ifs.gateway4 = None;
        }

        if let Some(addrs) = &nic_output.nameservers {
            for addr in addrs {
                if let Some(ifs_nameservers) = &mut ifs.nameservers {
                    for v in ifs_nameservers.values_mut() {
                        if v.contains(addr) {
                            v.retain(|x| *x != *addr);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    // TODO: synchronize /etc/netplan/--yaml vs nic running conf
    // pub fn sync(&self, _dir: &str) -> usize {
    //     0
    // }

    // Saves conf to netplan yaml file, and apply it to system. Merges all yaml files under /etc/netplan folder.
    //
    // The following errors are possible:
    //
    // * fail to get /etc/netplan yaml files
    // * fail to create or write temporary yaml file in /tmp
    // * fail to copy yaml file from /tmp to /etc/netplan
    // * fail to remove temporary file
    // * fail to remove /etc/netplan files except the first yaml file
    // * fail to run netplan apply command
    fn apply(&self, dir: &str) -> Result<()> {
        let files = list_files(dir, None, false)?;

        let mut from = format!("/tmp/{DEFAULT_NETPLAN_YAML}");
        let mut to = format!("{dir}/{DEFAULT_NETPLAN_YAML}");
        if let Some((_, _, first)) = files.first()
            && first != DEFAULT_NETPLAN_YAML
        {
            from = format!("/tmp/{first}");
            to = format!("{dir}/{first}");
        }

        let mut tmp = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&from)?;
        write!(tmp, "{self}")?;

        fs::copy(&from, &to)?;
        fs::remove_file(&from)?;

        for (_, _, file) in &files {
            let path = format!("{dir}/{file}");
            if path != to {
                fs::remove_file(&path)?;
            }
        }

        run_command("netplan", &["apply"])?;
        Ok(())
    }
}

// Gets all interface settings. Gets all netplan yaml conf from /etc/netplan and merge it into one.
//
// The following errors are possible:
//
// * fail to get yaml files from the /etc/netplan
// * fail to parse yaml file
// * yaml file not found
fn load_netplan_yaml(dir: &str) -> Result<NetplanYaml> {
    let files = list_files(dir, None, false)?;
    let mut netplan: Option<NetplanYaml> = None;
    for (_, _, file) in files {
        let path = format!("{dir}/{file}");
        let netplan_cfg = NetplanYaml::new(&path)?;
        if let Some(n) = &mut netplan {
            n.merge(netplan_cfg);
        } else {
            netplan = Some(netplan_cfg);
        }
    }
    if let Some(n) = netplan {
        Ok(n)
    } else {
        Err(anyhow!("Netplan configuration not found!"))
    }
}

fn validate_ipnetworks(ipnetwork: &str) -> Result<()> {
    match ipnetwork.parse::<IpNet>() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{e:?}")),
    }
}

fn validate_ipaddress(ipaddr: &str) -> Result<()> {
    match ipaddr.parse::<IpAddr>() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{e:?}")),
    }
}

// Initializes an interface.
//
// Be careful!. Netplan may remove address only in the yaml file.
// The addresess cab be remained in the running interface after netplan apply.
// To avoid this case, this function execute ifconfig system command internally.
//
// Possible errors:
// * interface name not found
// * fail to load /etc/netplan yaml files
// * fail to execute netplan apply
// * fail to ifconfig command
pub(crate) fn init(ifname: &str) -> Result<()> {
    let mut netplan = load_netplan_yaml(NETPLAN_PATH)?;
    let all_interfaces = interfaces();
    for iface in all_interfaces {
        if iface.name == *ifname {
            netplan.init_interface(ifname);
            netplan.apply(NETPLAN_PATH)?;

            // init running interface setting with ifconfig command
            // because 'netplan apply' command would not init the running settings.
            run_command("ifconfig", &[ifname, "0.0.0.0"])?;
            run_command("ifconfig", &[ifname, "up"])?;

            return Ok(());
        }
    }

    Err(anyhow!("interface \"{ifname}\" not found."))
}

// Sets interface ip address or gateway address or nameservers.
// This command will OVERWRITE all existing setting in the interface if exist.
//
// If the target interface is not running (cable connected), netplan does not
// set the address to interface. Instead it will just saved it into conf file.
//
// To replace(overwrite) ip address, gateway, nameservers of eno3 interface:
// let nic_output = NicOutput::new(
//     Some(vec!["192.168.0.205/24".to_string(), "192.168.4.7/24".to_string()]),
//     None,
//     Some("192.168.0.1".to_string()),
//     Some(vec!["164.124.101.1".to_string(), "164.124.101.2".to_string()])
// );
// ifconfig::set("eno3", &nic_output)?;
//
// Possible errors:
// * fail to get or save, apply netplan yaml conf
// * dhcp4 and static ip address or nameserver address is set in same interface
// * try to set new gateway address when other interface already have the gateway
pub(crate) fn set(ifname: &str, nic_output: &NicOutput) -> Result<()> {
    let mut netplan = load_netplan_yaml(NETPLAN_PATH)?;

    if let Some(addrs) = &nic_output.addresses {
        for ipnetwork in addrs {
            let res = validate_ipnetworks(ipnetwork);
            if let Err(e) = res {
                return Err(anyhow!("invalid interface address: {ipnetwork}. {e:?}"));
            }
        }
    }

    if let Some(ipaddr) = &nic_output.gateway4 {
        if let Err(e) = validate_ipaddress(ipaddr) {
            return Err(anyhow!("invalid gateway4 address: {ipaddr}. {e:?}"));
        }

        for (nic_name, nic) in &netplan.network.ethernets {
            if nic_name != ifname && nic.gateway4.is_some() {
                return Err(anyhow!("only one interface can have gateway."));
            }
        }
    }

    while let Some(ip) = &nic_output.nameservers {
        for ipaddr in ip {
            let res = validate_ipaddress(ipaddr);
            if let Err(e) = res {
                return Err(anyhow!("invalid nameserver address: {ipaddr}. {e:?}"));
            }
        }
    }

    if nic_output.dhcp4 == Some(true)
        && (nic_output.addresses.is_some() || nic_output.nameservers.is_some())
    {
        return Err(anyhow!(
            "dhcp4 and static address cannot be set in the same interface"
        ));
    }

    netplan.set_interface(ifname, nic_output.to());
    netplan.apply(NETPLAN_PATH)?;
    Ok(())
}

// Gets interface configurations
//
// To get all interfaces:
// let all_interfaces = ifconfig::get(&None)?;
//
// To get "eno1" interface:
// let eno1_interface = ifconfig::get(&Some("eno1".to_string()))?;
//
// Error: fail to load /etc/netplan yaml files
pub(crate) fn get(ifname: Option<&String>) -> Result<Option<Vec<(String, NicOutput)>>> {
    let netplan = load_netplan_yaml(NETPLAN_PATH)?;
    if let Some(name) = ifname {
        if let Some((_, nic)) = netplan.network.ethernets.iter().find(|(x, _)| *x == *name) {
            return Ok(Some(vec![(name.clone(), NicOutput::from(nic))]));
        }
    } else {
        let mut nic_output = Vec::new();
        for (name, nic) in &netplan.network.ethernets {
            nic_output.push((name.clone(), NicOutput::from(nic)));
        }
        return Ok(Some(nic_output));
    }
    Ok(None)
}

// Removes interface or name server or gateway address from the specified interface.
//
// To delete interface address "192.168.3.7/24", nameserver "164.124.101.2":
// let nic_output = NicOutput::new(
//     Some(vec!["192.168.3.7/24".to_string()]),
//     None,
//     None,
//     Some(vec!["164.124.101.2".to_string()]),);
//
// ifconfig::delete("eno3", &nic_output)?;
//
// Possible errors:
// * fail to load /etc/netplan yaml files
// * fail to apply the change to system
// * interface not found
pub(crate) fn delete(ifname: &str, nic_output: &NicOutput) -> Result<()> {
    let mut netplan = load_netplan_yaml(NETPLAN_PATH)?;
    netplan.delete(ifname, nic_output)?;
    netplan.apply(NETPLAN_PATH)?;

    if let Some(addrs) = &nic_output.addresses {
        for addr in addrs {
            // apply to running interface
            // if the device does not have this ip address, then this command will return ERROR!!!!
            run_command("ip", &["addr", "del", addr, "dev", ifname])?;
        }
    }
    Ok(())
}

// Gets interface names starting with the specified prefix.
// To get interface names starting with "en":
// let names = ifconfig::get_interface_names(&Some("en".to_string()));
#[must_use]
pub(crate) fn get_interface_names(arg: Option<&String>) -> Vec<String> {
    let mut nics = interfaces();
    if let Some(prefix) = arg {
        nics.retain(|f| f.name.starts_with(prefix));
    }
    nics.iter().map(|f| f.name.clone()).collect()
}

// Gets file list in the specified folder. No recursive into sub folder.
// Possible errors:
// * dir is not exist or fail to read dir
// * fail to get metadata from file
// * fail to get modified time from file
fn list_files(
    dir: &str,
    except: Option<&[&str]>,
    subdir: bool,
) -> Result<Vec<(u64, String, String)>> {
    let paths = fs::read_dir(dir)?;

    let mut files = Vec::new();
    for path in paths.flatten() {
        let filepath = path.path();
        let metadata = fs::metadata(filepath)?;
        let modified: DateTime<Local> = metadata.modified()?.into();

        if let Some(filename) = path.path().file_name()
            && let Some(filename) = filename.to_str()
        {
            if metadata.is_file() {
                files.push((
                    metadata.len(),
                    format!("{}", modified.format("%Y/%m/%d %T")),
                    filename.to_string(),
                ));
            } else if subdir && metadata.is_dir() {
                files.push((0, String::new(), filename.to_string()));
                /*
                // if it's required to traverse the directory recursively, uncomment this code
                if let Ok(ret) = list_files(filename, except, subdir) {
                    for (size, modified_time, name) in ret {
                        files.push((size, modified_time, format!("{}/{}", filename, name)));
                    }
                }
                */
            }
        }
    }
    if let Some(except) = except {
        for prefix in except {
            files.retain(|(_, _, name)| !name.starts_with(prefix));
        }
    }
    files.sort_by(|a, b| a.2.cmp(&b.2));
    Ok(files)
}

fn run_command(cmd: &str, args: &[&str]) -> Result<bool> {
    let status = Command::new(cmd)
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .args(args)
        .status()?;
    Ok(status.success())
}

#[cfg(test)]
mod tests {
    use std::{
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::*;

    fn make_temp_test_dir(label: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        dir.push(format!(
            "roxy-ifconfig-test-{label}-{}-{now}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).expect("should create temp test directory");
        dir
    }

    fn write_text_file(path: &Path, content: &str) {
        fs::write(path, content).expect("should write test yaml file");
    }

    // =========================================================================
    // NetplanYaml::new tests
    // =========================================================================

    #[test]
    fn netplan_new_reads_valid_yaml_file() {
        let dir = make_temp_test_dir("new-valid");
        let file = dir.join("01-netcfg.yaml");
        write_text_file(
            &file,
            r"network:
  version: 2
  renderer: networkd
  ethernets:
    eth0:
      addresses:
        - 10.0.0.1/24
      gateway4: 10.0.0.254
",
        );

        let netplan = NetplanYaml::new(file.to_str().expect("temp path should be valid utf-8"))
            .expect("valid yaml should parse");
        assert_eq!(netplan.network.version, Some(2));
        assert_eq!(netplan.network.renderer, Some("networkd".to_string()));
        assert_eq!(netplan.network.ethernets.len(), 1);
        assert_eq!(netplan.network.ethernets[0].0, "eth0");

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn netplan_new_fails_for_invalid_yaml() {
        let dir = make_temp_test_dir("new-invalid");
        let file = dir.join("broken.yaml");
        write_text_file(&file, "not: [valid");

        let err = NetplanYaml::new(file.to_str().expect("temp path should be valid utf-8"))
            .expect_err("invalid yaml should fail parsing");
        assert!(err.to_string().contains("Error:"));

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn netplan_new_fails_for_missing_file() {
        let dir = make_temp_test_dir("new-missing");
        let missing = dir.join("missing.yaml");

        let err = NetplanYaml::new(missing.to_str().expect("temp path should be valid utf-8"))
            .expect_err("missing file should fail");
        let io_err = err
            .downcast_ref::<std::io::Error>()
            .expect("missing file error should be std::io::Error");
        assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    // =========================================================================
    // load_netplan_yaml tests
    // =========================================================================

    #[test]
    fn load_netplan_yaml_merges_multiple_yaml_files() {
        let dir = make_temp_test_dir("load-merge");
        write_text_file(
            &dir.join("01-first.yaml"),
            r"network:
  version: 2
  renderer: networkd
  ethernets:
    eth1:
      addresses:
        - 10.0.1.10/24
",
        );
        write_text_file(
            &dir.join("02-second.yaml"),
            r"network:
  version: 3
  renderer: NetworkManager
  ethernets:
    eth0:
      addresses:
        - 10.0.0.10/24
",
        );

        let netplan = load_netplan_yaml(dir.to_str().expect("temp path should be valid utf-8"))
            .expect("loading valid yaml files should succeed");
        assert_eq!(netplan.network.version, Some(3));
        assert_eq!(netplan.network.renderer, Some("NetworkManager".to_string()));

        let names: Vec<&str> = netplan
            .network
            .ethernets
            .iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(names, vec!["eth0", "eth1"]);

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn load_netplan_yaml_fails_for_empty_directory() {
        let dir = make_temp_test_dir("load-empty");

        let err = load_netplan_yaml(dir.to_str().expect("temp path should be valid utf-8"))
            .expect_err("empty directory should fail");
        assert!(err.to_string().contains("Netplan configuration not found"));

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn load_netplan_yaml_fails_when_any_file_is_invalid() {
        let dir = make_temp_test_dir("load-invalid");
        write_text_file(
            &dir.join("01-valid.yaml"),
            r"network:
  version: 2
  renderer: networkd
  ethernets:
    eth0:
      addresses:
        - 10.0.0.1/24
",
        );
        write_text_file(&dir.join("02-invalid.yaml"), "network: [");

        let err = load_netplan_yaml(dir.to_str().expect("temp path should be valid utf-8"))
            .expect_err("invalid yaml file should fail loading");
        assert!(err.to_string().contains("Error:"));

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    // =========================================================================
    // list_files tests
    // =========================================================================

    #[test]
    fn list_files_lists_sorted_files_and_respects_except_prefixes() {
        let dir = make_temp_test_dir("list-files");
        write_text_file(&dir.join("b.yaml"), "b");
        write_text_file(&dir.join("a.yaml"), "a");
        fs::create_dir_all(dir.join("subdir")).expect("should create test subdirectory");

        let listed = list_files(
            dir.to_str().expect("temp path should be valid utf-8"),
            None,
            false,
        )
        .expect("listing files should succeed");
        let names: Vec<String> = listed.into_iter().map(|(_, _, name)| name).collect();
        assert_eq!(names, vec!["a.yaml".to_string(), "b.yaml".to_string()]);

        let filtered = list_files(
            dir.to_str().expect("temp path should be valid utf-8"),
            Some(&["a"]),
            false,
        )
        .expect("listing files with except should succeed");
        let filtered_names: Vec<String> = filtered.into_iter().map(|(_, _, name)| name).collect();
        assert_eq!(filtered_names, vec!["b.yaml".to_string()]);

        let with_subdir = list_files(
            dir.to_str().expect("temp path should be valid utf-8"),
            None,
            true,
        )
        .expect("listing files with subdir should succeed");
        let with_subdir_names: Vec<String> =
            with_subdir.into_iter().map(|(_, _, name)| name).collect();
        assert_eq!(
            with_subdir_names,
            vec![
                "a.yaml".to_string(),
                "b.yaml".to_string(),
                "subdir".to_string()
            ]
        );

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    #[test]
    fn list_files_fails_for_missing_directory() {
        let dir = make_temp_test_dir("list-missing");
        let missing = dir.join("no-such-dir");

        let err = list_files(
            missing.to_str().expect("temp path should be valid utf-8"),
            None,
            false,
        )
        .expect_err("listing missing directory should fail");
        let io_err = err
            .downcast_ref::<std::io::Error>()
            .expect("missing directory error should be std::io::Error");
        assert_eq!(io_err.kind(), std::io::ErrorKind::NotFound);

        fs::remove_dir_all(&dir).expect("temp test directory should be removable");
    }

    // =========================================================================
    // IP address validation tests (validate_ipaddress)
    // =========================================================================

    #[test]
    fn validate_ipaddress_accepts_valid_ipv4() {
        assert!(validate_ipaddress("192.0.2.1").is_ok());
        assert!(validate_ipaddress("0.0.0.0").is_ok());
        assert!(validate_ipaddress("255.255.255.255").is_ok());
        assert!(validate_ipaddress("127.0.0.1").is_ok());
    }

    #[test]
    fn validate_ipaddress_accepts_valid_ipv6() {
        assert!(validate_ipaddress("::1").is_ok());
        assert!(validate_ipaddress("::").is_ok());
        assert!(validate_ipaddress("2001:db8::1").is_ok());
        assert!(validate_ipaddress("fe80::1").is_ok());
        assert!(validate_ipaddress("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff").is_ok());
    }

    #[test]
    fn validate_ipaddress_rejects_empty_string() {
        assert!(validate_ipaddress("").is_err());
    }

    #[test]
    fn validate_ipaddress_rejects_non_numeric_chars() {
        assert!(validate_ipaddress("abc.def.ghi.jkl").is_err());
        assert!(validate_ipaddress("192.168.1.x").is_err());
        assert!(validate_ipaddress("hello").is_err());
    }

    #[test]
    fn validate_ipaddress_rejects_out_of_range_octets() {
        assert!(validate_ipaddress("256.0.0.1").is_err());
        assert!(validate_ipaddress("192.168.1.999").is_err());
    }

    #[test]
    fn validate_ipaddress_rejects_cidr_notation() {
        // validate_ipaddress expects plain IP, not CIDR
        assert!(validate_ipaddress("192.0.2.0/24").is_err());
        assert!(validate_ipaddress("::1/128").is_err());
    }

    // =========================================================================
    // CIDR/IP network validation tests (validate_ipnetworks)
    // =========================================================================

    #[test]
    fn validate_ipnetworks_accepts_valid_ipv4_cidr() {
        assert!(validate_ipnetworks("192.0.2.0/24").is_ok());
        assert!(validate_ipnetworks("10.0.0.0/8").is_ok());
        assert!(validate_ipnetworks("0.0.0.0/0").is_ok());
        assert!(validate_ipnetworks("192.168.1.1/32").is_ok());
    }

    #[test]
    fn validate_ipnetworks_accepts_valid_ipv6_cidr() {
        assert!(validate_ipnetworks("::/0").is_ok());
        assert!(validate_ipnetworks("::1/128").is_ok());
        assert!(validate_ipnetworks("2001:db8::/32").is_ok());
        assert!(validate_ipnetworks("fe80::1/64").is_ok());
    }

    #[test]
    fn validate_ipnetworks_rejects_missing_prefix() {
        assert!(validate_ipnetworks("192.0.2.0").is_err());
        assert!(validate_ipnetworks("::1").is_err());
    }

    #[test]
    fn validate_ipnetworks_rejects_non_numeric_prefix() {
        assert!(validate_ipnetworks("192.0.2.0/abc").is_err());
        assert!(validate_ipnetworks("192.0.2.0/").is_err());
    }

    #[test]
    fn validate_ipnetworks_rejects_out_of_range_ipv4_prefix() {
        assert!(validate_ipnetworks("192.0.2.0/33").is_err());
        assert!(validate_ipnetworks("192.0.2.0/-1").is_err());
    }

    #[test]
    fn validate_ipnetworks_rejects_out_of_range_ipv6_prefix() {
        assert!(validate_ipnetworks("::1/129").is_err());
    }

    #[test]
    fn validate_ipnetworks_rejects_malformed_input() {
        assert!(validate_ipnetworks("").is_err());
        assert!(validate_ipnetworks("not-an-ip/24").is_err());
        assert!(validate_ipnetworks("192.168.1/24").is_err());
    }

    // =========================================================================
    // NetplanYaml merge tests
    // =========================================================================

    fn make_netplan_with_bridges(
        ethernets: Vec<(String, Nic)>,
        bridges: Option<HashMap<String, Bridge>>,
    ) -> NetplanYaml {
        NetplanYaml {
            network: Network {
                version: Some(2),
                renderer: Some("networkd".to_string()),
                ethernets,
                bridges,
            },
        }
    }

    fn make_netplan(ethernets: Vec<(String, Nic)>) -> NetplanYaml {
        make_netplan_with_bridges(ethernets, None)
    }

    #[test]
    fn merge_adds_new_interface() {
        let mut base = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let new = make_netplan(vec![(
            "eth1".to_string(),
            Nic::new(
                Some(vec!["10.0.1.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        base.merge(new);

        assert_eq!(base.network.ethernets.len(), 2);
        assert!(base.network.ethernets.iter().any(|(n, _)| n == "eth0"));
        assert!(base.network.ethernets.iter().any(|(n, _)| n == "eth1"));
    }

    #[test]
    fn merge_overwrites_existing_interface() {
        let mut base = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let new = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["192.168.1.1/24".to_string()]),
                Some(false),
                Some("192.168.1.254".to_string()),
                None,
                None,
            ),
        )]);

        base.merge(new);

        assert_eq!(base.network.ethernets.len(), 1);
        let (_, nic) = &base.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["192.168.1.1/24".to_string()]));
        assert_eq!(nic.gateway4, Some("192.168.1.254".to_string()));
    }

    #[test]
    fn merge_updates_version_and_renderer() {
        let mut base = make_netplan(vec![]);
        let mut new = make_netplan(vec![]);
        new.network.version = Some(3);
        new.network.renderer = Some("NetworkManager".to_string());

        base.merge(new);

        assert_eq!(base.network.version, Some(3));
        assert_eq!(base.network.renderer, Some("NetworkManager".to_string()));
    }

    #[test]
    fn merge_sorts_interfaces_alphabetically() {
        let mut base = make_netplan(vec![(
            "eth2".to_string(),
            Nic::new(None, None, None, None, None),
        )]);

        let new = make_netplan(vec![
            ("eth0".to_string(), Nic::new(None, None, None, None, None)),
            ("eth1".to_string(), Nic::new(None, None, None, None, None)),
        ]);

        base.merge(new);

        let names: Vec<&str> = base
            .network
            .ethernets
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert_eq!(names, vec!["eth0", "eth1", "eth2"]);
    }

    #[test]
    fn merge_preserves_base_when_new_has_none_version_and_renderer() {
        let mut base = make_netplan(vec![]);
        let mut new = make_netplan(vec![]);
        new.network.version = None;
        new.network.renderer = None;

        base.merge(new);

        assert_eq!(base.network.version, Some(2));
        assert_eq!(base.network.renderer, Some("networkd".to_string()));
    }

    #[test]
    fn merge_overwrites_and_inserts_bridges() {
        let mut base_bridges = HashMap::new();
        base_bridges.insert(
            "br0".to_string(),
            Bridge {
                interfaces: vec!["eth0".to_string()],
                addresses: vec!["10.0.0.10/24".to_string()],
                gateway4: Some("10.0.0.1".to_string()),
                nameservers: Address {
                    search: None,
                    addresses: Some(vec!["8.8.8.8".to_string()]),
                },
            },
        );

        let mut new_bridges = HashMap::new();
        new_bridges.insert(
            "br0".to_string(),
            Bridge {
                interfaces: vec!["eth1".to_string()],
                addresses: vec!["192.168.1.10/24".to_string()],
                gateway4: Some("192.168.1.1".to_string()),
                nameservers: Address {
                    search: None,
                    addresses: Some(vec!["1.1.1.1".to_string()]),
                },
            },
        );
        new_bridges.insert(
            "br1".to_string(),
            Bridge {
                interfaces: vec!["eth2".to_string()],
                addresses: vec!["172.16.0.10/24".to_string()],
                gateway4: None,
                nameservers: Address {
                    search: Some(vec!["example.local".to_string()]),
                    addresses: Some(vec!["9.9.9.9".to_string()]),
                },
            },
        );

        let mut base = make_netplan_with_bridges(vec![], Some(base_bridges));
        let new = make_netplan_with_bridges(vec![], Some(new_bridges));

        base.merge(new);

        let bridges = base
            .network
            .bridges
            .as_ref()
            .expect("bridges should remain present after merge");
        assert_eq!(bridges.len(), 2);

        let br0 = bridges
            .get("br0")
            .expect("bridge br0 should be overwritten by new config");
        assert_eq!(br0.interfaces, vec!["eth1".to_string()]);
        assert_eq!(br0.gateway4, Some("192.168.1.1".to_string()));

        let br1 = bridges
            .get("br1")
            .expect("bridge br1 should be inserted from new config");
        assert_eq!(br1.addresses, vec!["172.16.0.10/24".to_string()]);
    }

    #[test]
    fn merge_preserves_existing_bridges_when_new_has_none() {
        let mut base_bridges = HashMap::new();
        base_bridges.insert(
            "br0".to_string(),
            Bridge {
                interfaces: vec!["eth0".to_string()],
                addresses: vec!["10.0.0.10/24".to_string()],
                gateway4: Some("10.0.0.1".to_string()),
                nameservers: Address {
                    search: Some(vec!["example.local".to_string()]),
                    addresses: Some(vec!["8.8.8.8".to_string()]),
                },
            },
        );

        let mut base = make_netplan_with_bridges(vec![], Some(base_bridges));
        let new = make_netplan_with_bridges(vec![], None);

        base.merge(new);

        let bridges = base
            .network
            .bridges
            .as_ref()
            .expect("base bridges should be preserved when new has none");
        assert_eq!(bridges.len(), 1);
        let br0 = bridges
            .get("br0")
            .expect("existing bridge should remain after merge");
        assert_eq!(br0.interfaces, vec!["eth0".to_string()]);
    }

    #[test]
    fn merge_preserves_unmentioned_existing_bridge_entries() {
        let mut base_bridges = HashMap::new();
        base_bridges.insert(
            "br0".to_string(),
            Bridge {
                interfaces: vec!["eth0".to_string()],
                addresses: vec!["10.0.0.10/24".to_string()],
                gateway4: Some("10.0.0.1".to_string()),
                nameservers: Address {
                    search: None,
                    addresses: Some(vec!["8.8.8.8".to_string()]),
                },
            },
        );
        base_bridges.insert(
            "br9".to_string(),
            Bridge {
                interfaces: vec!["eth9".to_string()],
                addresses: vec!["172.16.9.10/24".to_string()],
                gateway4: None,
                nameservers: Address {
                    search: Some(vec!["keep.local".to_string()]),
                    addresses: Some(vec!["9.9.9.9".to_string()]),
                },
            },
        );

        let mut new_bridges = HashMap::new();
        new_bridges.insert(
            "br0".to_string(),
            Bridge {
                interfaces: vec!["eth1".to_string()],
                addresses: vec!["192.168.1.10/24".to_string()],
                gateway4: Some("192.168.1.1".to_string()),
                nameservers: Address {
                    search: None,
                    addresses: Some(vec!["1.1.1.1".to_string()]),
                },
            },
        );

        let mut base = make_netplan_with_bridges(vec![], Some(base_bridges));
        let new = make_netplan_with_bridges(vec![], Some(new_bridges));
        base.merge(new);

        let bridges = base
            .network
            .bridges
            .as_ref()
            .expect("base bridges should remain present");
        assert_eq!(bridges.len(), 2);
        let br9 = bridges
            .get("br9")
            .expect("unmentioned existing bridge should not be removed");
        assert_eq!(br9.interfaces, vec!["eth9".to_string()]);
    }

    #[test]
    fn merge_with_duplicate_interface_entries_in_new_keeps_last_value() {
        let mut base = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let new = make_netplan(vec![
            (
                "eth0".to_string(),
                Nic::new(
                    Some(vec!["192.168.1.10/24".to_string()]),
                    None,
                    Some("192.168.1.1".to_string()),
                    None,
                    None,
                ),
            ),
            (
                "eth0".to_string(),
                Nic::new(
                    Some(vec!["192.168.1.20/24".to_string()]),
                    None,
                    Some("192.168.1.254".to_string()),
                    None,
                    None,
                ),
            ),
        ]);

        base.merge(new);

        assert_eq!(base.network.ethernets.len(), 1);
        let (_, nic) = &base.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["192.168.1.20/24".to_string()]));
        assert_eq!(nic.gateway4, Some("192.168.1.254".to_string()));
    }

    // =========================================================================
    // NetplanYaml delete tests
    // =========================================================================

    #[test]
    fn delete_removes_specified_address() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string(), "10.0.0.2/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let to_delete = NicOutput::new(Some(vec!["10.0.0.1/24".to_string()]), None, None, None);

        netplan
            .delete("eth0", &to_delete)
            .expect("deleting one address should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["10.0.0.2/24".to_string()]));
    }

    #[test]
    fn delete_removes_gateway_when_matching() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, Some("10.0.0.254".to_string()), None, None),
        )]);

        let to_delete = NicOutput::new(None, None, Some("10.0.0.254".to_string()), None);

        netplan
            .delete("eth0", &to_delete)
            .expect("deleting matching gateway should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.gateway4, None);
    }

    #[test]
    fn delete_preserves_gateway_when_not_matching() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, Some("10.0.0.254".to_string()), None, None),
        )]);

        let to_delete = NicOutput::new(None, None, Some("192.168.1.1".to_string()), None);

        netplan
            .delete("eth0", &to_delete)
            .expect("deleting non-matching gateway request should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.gateway4, Some("10.0.0.254".to_string()));
    }

    #[test]
    fn delete_removes_nameserver() {
        let mut nameservers = HashMap::new();
        nameservers.insert(
            "addresses".to_string(),
            vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
        );

        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, Some(nameservers), None),
        )]);

        let to_delete = NicOutput::new(None, None, None, Some(vec!["8.8.8.8".to_string()]));

        netplan
            .delete("eth0", &to_delete)
            .expect("deleting one nameserver should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        let ns = nic
            .nameservers
            .as_ref()
            .expect("nameservers should remain present after partial delete");
        let addrs = ns
            .get("addresses")
            .expect("addresses entry should remain after partial delete");
        assert_eq!(addrs, &vec!["1.1.1.1".to_string()]);
    }

    #[test]
    fn delete_fails_for_nonexistent_interface() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, None, None),
        )]);

        let to_delete = NicOutput::new(None, None, None, None);

        let err = netplan
            .delete("eth99", &to_delete)
            .expect_err("deleting unknown interface should fail");
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn delete_handles_empty_delete_request() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                Some("10.0.0.254".to_string()),
                None,
                None,
            ),
        )]);

        let to_delete = NicOutput::new(None, None, None, None);

        netplan
            .delete("eth0", &to_delete)
            .expect("empty delete request should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["10.0.0.1/24".to_string()]));
        assert_eq!(nic.gateway4, Some("10.0.0.254".to_string()));
    }

    #[test]
    fn delete_preserves_addresses_when_requested_address_not_present() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let to_delete = NicOutput::new(Some(vec!["10.0.0.99/24".to_string()]), None, None, None);
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting absent address should still succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["10.0.0.1/24".to_string()]));
    }

    #[test]
    fn delete_keeps_addresses_none_when_interface_has_no_addresses() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, None, None),
        )]);

        let to_delete = NicOutput::new(Some(vec!["10.0.0.1/24".to_string()]), None, None, None);
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting address from none should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert!(nic.addresses.is_none());
    }

    #[test]
    fn delete_leaves_empty_address_vec_when_all_addresses_removed() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let to_delete = NicOutput::new(Some(vec!["10.0.0.1/24".to_string()]), None, None, None);
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting last address should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert!(nic.addresses.as_ref().is_some_and(Vec::is_empty));
    }

    #[test]
    fn delete_removes_matching_value_from_all_nameserver_keys() {
        let mut nameservers = HashMap::new();
        nameservers.insert("addresses".to_string(), vec!["shared".to_string()]);
        nameservers.insert(
            "search".to_string(),
            vec!["shared".to_string(), "keep".to_string()],
        );

        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, Some(nameservers), None),
        )]);

        let to_delete = NicOutput::new(None, None, None, Some(vec!["shared".to_string()]));
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting nameserver should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        let ns = nic
            .nameservers
            .as_ref()
            .expect("nameservers map should remain present");
        let addresses = ns
            .get("addresses")
            .expect("addresses key should still exist after delete");
        let search = ns
            .get("search")
            .expect("search key should still exist after delete");
        assert!(addresses.is_empty());
        assert_eq!(search, &vec!["keep".to_string()]);
    }

    #[test]
    fn delete_preserves_nameservers_when_value_not_present() {
        let mut nameservers = HashMap::new();
        nameservers.insert(
            "addresses".to_string(),
            vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()],
        );

        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, Some(nameservers), None),
        )]);

        let to_delete = NicOutput::new(None, None, None, Some(vec!["9.9.9.9".to_string()]));
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting absent nameserver should still succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        let ns = nic
            .nameservers
            .as_ref()
            .expect("nameservers should remain unchanged");
        let addresses = ns
            .get("addresses")
            .expect("addresses key should remain unchanged");
        assert_eq!(
            addresses,
            &vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()]
        );
    }

    #[test]
    fn delete_keeps_nameservers_none_when_interface_has_no_nameservers() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(None, None, None, None, None),
        )]);

        let to_delete = NicOutput::new(None, None, None, Some(vec!["8.8.8.8".to_string()]));
        netplan
            .delete("eth0", &to_delete)
            .expect("deleting nameserver from none should succeed");

        let (_, nic) = &netplan.network.ethernets[0];
        assert!(nic.nameservers.is_none());
    }

    // =========================================================================
    // NetplanYaml set_interface tests
    // =========================================================================

    #[test]
    fn set_interface_updates_existing() {
        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let new_nic = Nic::new(
            Some(vec!["192.168.1.1/24".to_string()]),
            Some(false),
            Some("192.168.1.254".to_string()),
            None,
            None,
        );

        netplan.set_interface("eth0", new_nic);

        assert_eq!(netplan.network.ethernets.len(), 1);
        let (_, nic) = &netplan.network.ethernets[0];
        assert_eq!(nic.addresses, Some(vec!["192.168.1.1/24".to_string()]));
        assert_eq!(nic.gateway4, Some("192.168.1.254".to_string()));
    }

    #[test]
    fn set_interface_adds_new_and_sorts() {
        let mut netplan = make_netplan(vec![(
            "eth1".to_string(),
            Nic::new(None, None, None, None, None),
        )]);

        let new_nic = Nic::new(
            Some(vec!["10.0.0.1/24".to_string()]),
            None,
            None,
            None,
            None,
        );

        netplan.set_interface("eth0", new_nic);

        assert_eq!(netplan.network.ethernets.len(), 2);
        let names: Vec<&str> = netplan
            .network
            .ethernets
            .iter()
            .map(|(n, _)| n.as_str())
            .collect();
        assert_eq!(names, vec!["eth0", "eth1"]);
    }

    // =========================================================================
    // NetplanYaml init_interface tests
    // =========================================================================

    #[test]
    fn init_interface_creates_empty_config() {
        let mut netplan = make_netplan(vec![]);

        netplan.init_interface("eth0");

        assert_eq!(netplan.network.ethernets.len(), 1);
        let (name, nic) = &netplan.network.ethernets[0];
        assert_eq!(name, "eth0");
        assert!(nic.addresses.is_none());
        assert!(nic.dhcp4.is_none());
        assert!(nic.gateway4.is_none());
        assert!(nic.nameservers.is_none());
    }

    #[test]
    fn init_interface_resets_existing_interface_fields() {
        let mut nameservers = HashMap::new();
        nameservers.insert("addresses".to_string(), vec!["8.8.8.8".to_string()]);

        let mut netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                Some(false),
                Some("10.0.0.254".to_string()),
                Some(nameservers),
                None,
            ),
        )]);

        netplan.init_interface("eth0");

        let (_, nic) = &netplan.network.ethernets[0];
        assert!(nic.addresses.is_none());
        assert!(nic.dhcp4.is_none());
        assert!(nic.gateway4.is_none());
        assert!(nic.nameservers.is_none());
    }

    // =========================================================================
    // NetplanYaml Display tests
    // =========================================================================

    #[test]
    fn netplan_yaml_display_produces_yaml() {
        let netplan = make_netplan(vec![(
            "eth0".to_string(),
            Nic::new(
                Some(vec!["10.0.0.1/24".to_string()]),
                None,
                None,
                None,
                None,
            ),
        )]);

        let output = netplan.to_string();

        assert!(output.contains("network:"));
        assert!(output.contains("ethernets:"));
        assert!(output.contains("eth0:"));
        assert!(output.contains("10.0.0.1/24"));
    }
}
