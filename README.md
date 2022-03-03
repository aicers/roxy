# Roxy

Roxy is a root proxy that executes a system command requiring the root
privilege.

* Roxy should be saved with `setuid` enabled and it's owner and group should be `root`.
  * `chown root.root roxy`
  * `chmod u+s roxy`

* Version format in `/etc/version` file

```text
  OS: AICE OS v1.0.9
  Product: AICE security v1.2.0
```

* To control machine, following utilities and files are used
  * utilities
    * df
    * ip
    * netplan
    * systemctl (ntp, rsyslog, sshd)
    * ufw
    * uptime
  * files
    * /etc/netplan/01-netcfg.yaml
    * /etc/ntp.conf
    * /etc/rsyslog.d/50-default.conf
    * /etc/ssh/sshd_config
    * /etc/version

* To find utilities, following path will be searched
  * /usr/bin
  * /usr/sbin
  * /bin
  * /sbin

* Roxy is supposed to be located in "/usr/local/aice/bin"

* Tips for services
  * netplan, ip
    * netplan did not set ip address for a interface if it's not running. This can cause an error when delete ip address.
    * Sometimes netplan did not remove ip address when **netplan apply** command executed with conf ip address removed.
      * Few lines of code are added to solve this problem.
      * **ip** command is used to do this.

        ```bash
        ip addr del <ip-address/prefixlen> dev <interface-name>
        ```

  * ntp
    * all **"pool ?.ubuntu.pool.ntp.org iburst"** or **"pool x.x.x.x"** lines should be deleted as a default except appended things by Roxy
    * Roxy will add new ntp server or replace it

      ```text
      server new.ntpserver.from.webui iburst
      ```

  * sshd
    * New lines will be appended or replaced if exist at the end of **/etc/ssh/sshd_config**

      ```text
      Port 10022
      ```

  * rsyslog
    * New remote syslog server will be appended or replaced at the end of **/etc/rsyslogd/50-default.conf**

      ```text
      user.*    @@192.168.0.2:7500
      user.*     @192.168.0.3:500
      ```

  * ufw
    * To enable or disable ufw, **ufw enable/disable** command will be used instead of **systemctl**
    * **systemctl** did not detect ufw status exactly
