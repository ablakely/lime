# NOTE: If you plan to use this, consider setting `proxy_buffers 256
# 4k` and `proxy_max_temp_file_size 0` in nginx to avoid thrashing
# your ssd

{ config, pkgs, lib, ... }:

let lemon-website = pkgs.callPackage (import ./package.nix) {};
    types = lib.types;
    unixSocket = prefix: "/run/lemon-website/${prefix}.socket";
    mergeIfEnabled = f: lib.mkMerge (lib.mapAttrsToList (prefix: cfg: lib.mkIf cfg.enable (f prefix cfg)) config.services.lemon-website);
    anyNginxEnabled = builtins.any (s: s.nginx.enable) (builtins.attrValues config.services.lemon-website);
    username = "lemon-website";
in

{
  options.services.lemon-website = lib.mkOption {
    description = "Attr names are unique ids, should all be lowercase english letters to be safe.";
    default = {};
    type = types.attrsOf (types.submodule ({ config, ... }: {
      options = {
        enable = lib.mkEnableOption "Enable LEMON website";
        package = lib.mkOption {
          type = types.package;
          default = lemon-website;
          description = "lemon-website package";
        };
        indexJsons = lib.mkOption {
          type = types.listOf types.path;
          default = [];
          description = "list of paths to index.json files";
        };
        domains = lib.mkOption {
          type = types.listOf types.str;
          default = [];
          example = "[ \"example.com\" ]";
          description = "list of domain names. Informs both the website display and nginx virtualhosts";
        };
        nginx = {
          enable = lib.mkOption {
            type = types.bool;
            default = false;
            description = "whether to configure nginx to serve lemon.";
          };
        };
      };
    }));
  };

  config.users = mergeIfEnabled (prefix: cfg: {
    users.${username} = {
      isSystemUser = true;
      group = username;
    };
    groups.${username} = {};
    users.nginx.extraGroups = lib.mkIf cfg.nginx.enable [ username ];
  });

  config.services.nginx.additionalModules = lib.mkIf anyNginxEnabled [ pkgs.nginxModules.brotli ];

  config.services.nginx.virtualHosts = mergeIfEnabled (prefix: cfg: lib.mkIf cfg.nginx.enable (
    builtins.listToAttrs (map (domain: {
      name = domain;
      value = {
        locations."/" = {
          proxyPass = "http://unix:${unixSocket prefix}:";
        };

        extraConfig = ''
          brotli on;
          brotli_static on;
          brotli_comp_level 5;
          brotli_window 512k;
          brotli_min_length 256;
          brotli_types text/html text/css text/javascript application/javascript image/svg+xml;

          gzip on;
          gzip_static on;
          gzip_vary on;
          gzip_comp_level 5;
          gzip_min_length 256;
          gzip_proxied expired no-cache no-store private auth;
          gzip_types text/html text/css text/javascript application/javascript image/svg+xml;

          proxy_set_header Host $host;
          proxy_set_header X-Real-IP $remote_addr;
          proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
          proxy_set_header X-Forwarded-Proto $scheme;
        '';
      };
    }) cfg.domains)
  ));

  config.systemd = mergeIfEnabled (prefix: cfg: {
    services."lemon-website-${prefix}" = {
      script = ''
            ${cfg.package}/bin/lemon-website \
              --production \
              --listen-address unix:${unixSocket prefix} \
              ${builtins.toString cfg.indexJsons} 
          '';
      wantedBy = [ "multi-user.target" ];
      startLimitIntervalSec = 60;
      startLimitBurst = 2;
      restartTriggers = [ cfg.package ];
      serviceConfig = {
        Restart = "always";
        User = username;
        Group = username;

        RestrictNamespaces = true;
        ProtectControlGroups = true;
        ProtectKernelModules = true;
        ProtectKernelTunables = true;
        ProtectHostname = true;
        LockPersonality = true;
      };
    };
    tmpfiles.settings.lemon-website-run."/run/lemon-website".d = {
      user = username;
      group = username;
    };
  });
}
