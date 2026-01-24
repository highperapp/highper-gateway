#cloud-config
# Highper Gateway - Azure Cloud-Init Configuration
# Use Case: ${use_case}
# Environment: ${environment}

package_update: true
package_upgrade: true

packages:
  - curl
  - wget
  - ca-certificates
  - gnupg
  - liburing2
  - liburing-dev
  - jq

write_files:
  - path: /etc/security/limits.d/highper-gateway.conf
    content: |
      # Highper Gateway io_uring limits
      *               soft    memlock         unlimited
      *               hard    memlock         unlimited
      *               soft    nofile          1048576
      *               hard    nofile          1048576
      *               soft    nproc           unlimited
      *               hard    nproc           unlimited
    permissions: '0644'

  - path: /etc/sysctl.d/99-highper-gateway.conf
    content: |
      # Network performance tuning for Highper Gateway
      net.core.rmem_max = 134217728
      net.core.wmem_max = 134217728
      net.core.rmem_default = 16777216
      net.core.wmem_default = 16777216
      net.ipv4.tcp_rmem = 4096 87380 134217728
      net.ipv4.tcp_wmem = 4096 65536 134217728
      net.core.somaxconn = 65535
      net.core.netdev_max_backlog = 65535
      net.ipv4.tcp_max_syn_backlog = 65535
      net.ipv4.tcp_fastopen = 3
      net.ipv4.tcp_slow_start_after_idle = 0
      net.ipv4.tcp_no_metrics_save = 1
      net.ipv4.tcp_syncookies = 1
      net.ipv4.tcp_tw_reuse = 1
      net.ipv4.tcp_keepalive_time = 60
      net.ipv4.tcp_keepalive_intvl = 10
      net.ipv4.tcp_keepalive_probes = 6
      net.core.default_qdisc = fq
      net.ipv4.tcp_congestion_control = bbr
      fs.file-max = 2097152
      fs.nr_open = 2097152
      vm.swappiness = 10
      vm.dirty_ratio = 60
      vm.dirty_background_ratio = 2
    permissions: '0644'

  - path: /etc/highper-gateway/config.yaml
    content: |
      # Highper Gateway Configuration
      # Use Case: ${use_case}
      server:
        name: "highper-gateway-${environment}"
      io_uring:
        entries: 4096
        sq_poll: true
        sq_poll_cpu: 0
      logging:
        level: info
        format: json
        output: /var/log/highper-gateway/gateway.log
      metrics:
        enabled: true
        endpoint: /metrics
        port: 9090
      health:
        enabled: true
        endpoint: /health
        port: 8081
    permissions: '0640'

  - path: /etc/systemd/system/highper-gateway.service
    content: |
      [Unit]
      Description=Highper Gateway - High Performance Reverse Proxy
      Documentation=https://highper-gateway.io/docs
      After=network-online.target
      Wants=network-online.target

      [Service]
      Type=exec
      User=highper-gateway
      Group=highper-gateway
      ExecStart=/usr/bin/highper-gateway --config /etc/highper-gateway/config.yaml
      ExecReload=/bin/kill -HUP $MAINPID
      Restart=on-failure
      RestartSec=5
      CapabilityBoundingSet=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
      AmbientCapabilities=CAP_NET_BIND_SERVICE CAP_IPC_LOCK
      LimitMEMLOCK=infinity
      LimitNOFILE=1048576
      LimitNPROC=infinity
      NoNewPrivileges=yes
      ProtectSystem=strict
      ProtectHome=yes
      PrivateTmp=yes
      ProtectKernelTunables=yes
      ProtectKernelModules=yes
      ProtectControlGroups=yes
      StandardOutput=append:/var/log/highper-gateway/gateway.log
      StandardError=append:/var/log/highper-gateway/error.log
      WorkingDirectory=/var/lib/highper-gateway
      Environment=RUST_LOG=info
      Environment=RUST_BACKTRACE=1

      [Install]
      WantedBy=multi-user.target
    permissions: '0644'

  - path: /etc/logrotate.d/highper-gateway
    content: |
      /var/log/highper-gateway/*.log {
          daily
          rotate 14
          compress
          delaycompress
          missingok
          notifempty
          create 0640 highper-gateway highper-gateway
          sharedscripts
          postrotate
              systemctl reload highper-gateway > /dev/null 2>&1 || true
          endscript
      }
    permissions: '0644'

runcmd:
  - sysctl -p /etc/sysctl.d/99-highper-gateway.conf
  - mkdir -p /etc/highper-gateway/{certs,rules,backends}
  - mkdir -p /var/log/highper-gateway
  - mkdir -p /var/lib/highper-gateway
  - useradd --system --shell /usr/sbin/nologin --home /var/lib/highper-gateway highper-gateway || true
  - chown -R highper-gateway:highper-gateway /etc/highper-gateway
  - chown -R highper-gateway:highper-gateway /var/log/highper-gateway
  - chown -R highper-gateway:highper-gateway /var/lib/highper-gateway
  - chmod 750 /etc/highper-gateway
  - systemctl daemon-reload
  - systemctl enable highper-gateway
  - echo "Highper Gateway setup completed - Use case: ${use_case}, Environment: ${environment}"
