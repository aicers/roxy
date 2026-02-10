# Roxy

Roxy is a root proxy that executes a system command requiring the root privilege.

- The Roxy binary, `roxy`, should be owned by root with its `setuid` flag set.
  It should also belong to the same group as the services invoking roxy, e.g.
  "roxy", and the group should have execute permission. Note that "other" should
  not have execute permission.

  ```sh
  chown root:roxy roxy
  chmod 750 roxy
  chmod u+s roxy
  ```

- Version format in `/etc/version` file

  ```text
    OS: AICE OS v1.0.9
    Product: AICE security v1.2.0
  ```

- To control machine, following utilities and files are used

  - utilities
    - ip
    - netplan
    - systemctl (ntp, rsyslog, sshd)
    - ufw
  - files
    - /etc/netplan/01-netcfg.yaml
    - /etc/ntp.conf
    - /etc/rsyslog.d/50-default.conf
    - /etc/ssh/sshd_config
    - /etc/version

- To find utilities, following path will be searched

  - /usr/bin
  - /usr/sbin
  - /bin
  - /sbin

- Roxy is supposed to be located in "/opt/clumit/bin"

- Tips for services

  - netplan, ip

    - netplan did not set ip address for a interface if it's not running. This
      can cause an error when delete ip address.
    - Sometimes netplan did not remove ip address when **netplan apply** command
      executed with conf ip address removed.

      - Few lines of code are added to solve this problem.
      - **ip** command is used to do this.

        ```bash
        ip addr del <ip-address/prefixlen> dev <interface-name>
        ```

  - ntp

    - all **"pool ?.ubuntu.pool.ntp.org iburst"** or **"pool x.x.x.x"** lines
      should be deleted as a default except appended things by Roxy
    - Roxy will add new ntp server or replace it

      ```text
      server new.ntpserver.from.webui iburst
      ```

  - sshd

    - New lines will be appended or replaced if exist at the end of
      **/etc/ssh/sshd_config**

      ```text
      Port 10022
      ```

  - rsyslog

    - New remote syslog server will be appended or replaced at the end of
      **/etc/rsyslogd/50-default.conf**

      ```text
      user.*    @@192.168.0.2:7500
      user.*     @192.168.0.3:500
      ```

  - ufw
    - To enable or disable ufw, **ufw enable/disable** command will be used
      instead of **systemctl**
    - **systemctl** did not detect ufw status exactly

## roxyd (experimental)

`roxyd` is a new implementation path that coexists with the legacy `roxy`
binary. It is designed to connect to the Manager via QUIC with mTLS
authentication.

**Important notes:**

- This is currently a skeleton implementation with no protocol handlers active.
- Existing legacy code **must not** be removed while legacy mode is still in use.
- Current limitations: skeleton only; no review-protocol request handling yet.

### Running roxyd

```sh
cargo run --bin roxyd -- -c path/to/config.toml \
  --cert path/to/cert.pem \
  --key path/to/key.pem --ca-certs path/to/ca.pem \
  manager@192.168.1.100:4433
```

### Configuration

Create a TOML configuration file with the following structure.

```toml
# Path to the log file. If omitted, logs go to stdout.
log_path = "/opt/clumit/log/roxyd.log"
```

Configuration can also be overridden using environment variables with the
`ROXYD_` prefix (e.g., `ROXYD_LOG_PATH`).

## License

Copyright 2022-2024 ClumL Inc.

Licensed under [Apache License, Version 2.0][apache-license] (the "License");
you may not use this crate except in compliance with the License.

Unless required by applicable law or agreed to in writing, software distributed
under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
CONDITIONS OF ANY KIND, either express or implied. See [LICENSE](LICENSE) for
the specific language governing permissions and limitations under the License.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the [Apache-2.0 license][apache-license],
shall be licensed as above, without any additional terms or conditions.

[apache-license]: http://www.apache.org/licenses/LICENSE-2.0
