{
  config,
  lib,
  pkgs,
  ...
}: let
  inherit (builtins) toJSON;
  inherit (lib.attrsets) mapAttrsToList optionalAttrs;
  inherit (lib.meta) getExe;
  inherit (lib.lists) optional unique;
  inherit (lib.modules) mkDefault mkIf;
  inherit (lib.options) mkEnableOption mkOption mkPackageOption;
  inherit (lib.strings) concatStringsSep hasPrefix optionalString removePrefix removeSuffix replaceStrings;
  inherit (lib.types) attrsOf bool int lines listOf oneOf path port str submodule nullOr;

  json = pkgs.formats.json {};
  toml = pkgs.formats.toml {};
  nodeConfigDirectory = name: defaultDirectory: settings:
    pkgs.runCommand "${name}-config" {} ''
      mkdir "$out"
      cp --recursive --symbolic-link ${defaultDirectory}/. "$out"
      ln --symbolic ${json.generate "${name}.json" settings} "$out/${name}.json"
    '';

  redisPort = toString config.services.redis.servers.zotero-dataserver.port;

  provisionUnit = "zotero-dataserver-provision.service";
  generatedCredentialDir = "%S/zotero-dataserver";
  portalDatabaseUrl = "mysql://zotero-portal@localhost/zotero_www?socket=${toString config.services.mysql.settings.mysqld.socket}";
  portalDatabaseUrlFile =
    if cfg.portal.database.urlFile != null
    then toString cfg.portal.database.urlFile
    else pkgs.writeText "zotero-portal-database-url" portalDatabaseUrl;

  restartAlways = {
    Restart = "always";
    RestartSec = 5;
  };
  restartOnFailure = restartAlways // {Restart = "on-failure";};

  portalLoggingEnvironment = {
    RUST_LOG = cfg.portal.logging.filter;
  };

  minimalOpenSearch = pkgs.opensearch.overrideAttrs (old: {
    pname = "opensearch-minimal";
    postInstall =
      (old.postInstall or "")
      + ''
        find "$out/plugins" -mindepth 1 -maxdepth 1 \
          ! -name opensearch-security -exec rm -r -- {} +
      '';
  });

  credentialFiles = {
    authSalt = "${generatedCredentialDir}/AUTH_SALT";
    attachmentProxySecret = "${generatedCredentialDir}/ATTACHMENT_PROXY_SECRET";
    dataserverSuperPassword = "${generatedCredentialDir}/DATASERVER_SUPER_PASSWORD";
    portalCookieKey = "${generatedCredentialDir}/PORTAL_COOKIE_KEY";
    s3AccessKeyId =
      if cfg.storage.accessKeyIdFile != null
      then cfg.storage.accessKeyIdFile
      else "${generatedCredentialDir}/S3_ACCESS_KEY_ID";
    s3SecretAccessKey =
      if cfg.storage.secretAccessKeyFile != null
      then cfg.storage.secretAccessKeyFile
      else "${generatedCredentialDir}/S3_SECRET_ACCESS_KEY";
  };

  phpBool = value:
    if value
    then "true"
    else "false";
  phpString = value: "'${replaceStrings ["\\" "'"] ["\\\\" "\\'"] value}'";
  phpStringList = values: "[${concatStringsSep ", " (map phpString values)}]";
  phpStringAttrs = values: "[${concatStringsSep ", " (mapAttrsToList (name: value: "${phpString name} => ${phpString value}") values)}]";
  phpStringListAttrs = values: "[${concatStringsSep ", " (mapAttrsToList (name: value: "${phpString name} => ${phpStringList value}") values)}]";
  settingOption = type: default: description:
    mkOption {inherit type default description;};
  microserviceSettings = name: options:
    mkOption {
      type = submodule {
        freeformType = json.type;
        inherit options;
      };
      default = {};
      description = "${name} configuration. Undeclared attributes are passed through as JSON.";
    };

  normalizeUri = uri: "${removeSuffix "/" uri}/";
  normalizePath = path: "/${removePrefix "/" (removeSuffix "/" path)}/";
  virtualHostUri =
    if hasPrefix "http://" cfg.virtualHost || hasPrefix "https://" cfg.virtualHost
    then cfg.virtualHost
    else "https://${cfg.virtualHost}";
  publicBaseUri = normalizeUri virtualHostUri;
  publicApiBaseUri = "${removeSuffix "/" publicBaseUri}${cfg.apiPath}";
  fulltextIndexName = "item_fulltext_index";
  fulltextIndexUrl = "http://127.0.0.1:${toString cfg.fulltext-search.settings.port}/${fulltextIndexName}";
  fulltextIndex = json.generate "zotero-fulltext-index.json" cfg.fulltext-search.index;

  defaultConfig = ''
    <?
    class Z_CONFIG {
      public static $API_ENABLED = ${phpBool cfg.php.settings.apiEnabled};
      public static $READ_ONLY = ${phpBool cfg.php.settings.readOnly};
      public static $MAINTENANCE_MESSAGE = ${phpString cfg.php.settings.maintenanceMessage};
      public static $BACKOFF = ${toString cfg.php.settings.backoff};

      public static $TESTING_SITE = ${phpBool cfg.php.settings.testingSite};
      public static $DEV_SITE = ${phpBool cfg.php.settings.devSite};

      public static $DEBUG_LOG = ${phpBool cfg.php.settings.debugLog};

      public static $BASE_URI = ${phpString cfg.php.settings.baseUri};
      public static $API_BASE_URI = ${phpString publicApiBaseUri};
      public static $WWW_BASE_URI = ${phpString publicBaseUri};
      public static $REPROXY_MAP = ${phpStringListAttrs cfg.php.settings.reproxyMap};
      public static $URI_PREFIX_DOMAIN_MAP = ${phpStringAttrs cfg.php.settings.uriPrefixDomainMap};

      public static $AUTH_SALT = "";
      public static $API_SUPER_USERNAME = 'zotero-portal';
      public static $API_SUPER_PASSWORD = "";

      public static $AWS_REGION = ${phpString cfg.storage.region};
      public static $AWS_ACCESS_KEY = ""; // leave credentials empty to use IAM role
      public static $AWS_SECRET_KEY = "";
      public static $S3_ENDPOINT = ${phpString cfg.storage.endpoint};
      public static $S3_FORCE_PATH_STYLE = ${phpBool cfg.storage.forcePathStyle};
      public static $S3_UPLOAD_BASE_URL = ${phpString cfg.storage.uploadBaseUrl};
      public static $S3_BUCKET = ${phpString cfg.storage.bucket};
      public static $S3_BUCKET_CACHE = ${phpString cfg.storage.cacheBucket};
      public static $S3_BUCKET_FULLTEXT = ${phpString cfg.storage.fulltextBucket};
      public static $S3_BUCKET_ERRORS = ${phpString cfg.storage.errorsBucket};
      public static $SNS_ALERT_TOPIC = ${phpString cfg.php.settings.snsAlertTopic};

      public static $REDIS_HOSTS = [
        'default' => [
          'host' => '127.0.0.1:${redisPort}'
        ],
        'request-limiter' => [
          'host' => '127.0.0.1:${redisPort}'
        ],
        'notifications' => [
          'host' => '127.0.0.1:${redisPort}'
        ],
        'fulltext-migration' => [
          'host' => '127.0.0.1:${redisPort}',
        ]
      ];

      public static $REDIS_PREFIX = ${phpString cfg.php.settings.redisPrefix};

      public static $MEMCACHED_ENABLED = true;
      public static $MEMCACHED_SERVERS = [
        "localhost:${toString config.services.memcached.port}"
      ];

      public static $TRANSLATION_SERVERS = [
        "http://127.0.0.1:${toString cfg.translation-server.settings.port}"
      ];

      public static $CITATION_SERVERS = [
        "127.0.0.1:${toString cfg.citeproc-js-server.settings.port}"
      ];

      public static $SEARCH_HOSTS = ${toJSON cfg.php.settings.searchHosts};

      public static $GLOBAL_ITEMS_URL = ${phpString cfg.php.settings.globalItemsUrl};

      public static $ATTACHMENT_PROXY_URL = ${phpString cfg.attachment-proxy.url};
      public static $ATTACHMENT_PROXY_SECRET = "";
      public static $ATTACHMENT_PROXY_ALWAYS = ${phpBool cfg.storage.proxyAllTransfers};

      public static $TTS_TABLE = "TTS";
      public static $S3_BUCKET_TTS = ${phpString cfg.storage.ttsBucket};
      public static $TTS_AUDIO_DOMAIN = ${phpString cfg.php.settings.ttsAudioDomain};
      public static $TTS_CREDIT_LIMITS = [
        'standard' => ['free' => 240, 'personal' => 999999, 'institutional' => 999999],
        'premium' => ['free' => 150, 'personal' => 3000, 'institutional' => 300],
      ];
      public static $TTS_DAILY_LIMIT_MINUTES = 480; // 8 hours -- safety valve for runaway tabs
      public static $TTS_DEV_USERS = ${toJSON cfg.php.settings.ttsDevUsers};

      public static $STATSD_ENABLED = ${phpBool cfg.php.settings.statsd.enable};
      public static $STATSD_PREFIX = ${phpString cfg.php.settings.statsd.prefix};
      public static $STATSD_HOST = ${phpString cfg.php.settings.statsd.host};
      public static $STATSD_PORT = ${toString cfg.php.settings.statsd.port};

      public static $LOG_TO_SCRIBE = ${phpBool cfg.php.settings.logToScribe};
      public static $LOG_ADDRESS = ${phpString cfg.php.settings.logAddress};
      public static $LOG_PORT = ${toString cfg.php.settings.logPort};
      public static $LOG_TIMEZONE = ${phpString cfg.php.settings.logTimezone};
      public static $LOG_TARGET_DEFAULT = ${phpString cfg.php.settings.logTarget};

      public static $HTMLCLEAN_SERVER_URL = ${phpString "http://127.0.0.1:${toString cfg.tinymce-clean-server.settings.port}"};

      // Set some things manually for running via command line
      public static $CLI_PHP_PATH = ${phpString (getExe cfg.package.php)};

      // Alternative to S3_BUCKET_ERRORS
      public static $ERROR_PATH = ${phpString cfg.php.settings.errorPath};

      public static $CACHE_VERSION_ATOM_ENTRY = 1;
      public static $CACHE_VERSION_BIB = 1;
      public static $CACHE_VERSION_RESPONSE_JSON_COLLECTION = 1;
      public static $CACHE_VERSION_RESPONSE_JSON_ITEM = 1;
      public static $CACHE_ENABLED_ITEM_RESPONSE_JSON = true;

    }

    $credentialDirectory = getenv('CREDENTIALS_DIRECTORY');
    Z_CONFIG::$AUTH_SALT = trim(file_get_contents($credentialDirectory . '/AUTH_SALT'));
    Z_CONFIG::$ATTACHMENT_PROXY_SECRET = trim(file_get_contents($credentialDirectory . '/ATTACHMENT_PROXY_SECRET'));
    Z_CONFIG::$AWS_ACCESS_KEY = trim(file_get_contents($credentialDirectory . '/S3_ACCESS_KEY_ID'));
    Z_CONFIG::$AWS_SECRET_KEY = trim(file_get_contents($credentialDirectory . '/S3_SECRET_ACCESS_KEY'));
    Z_CONFIG::$API_SUPER_PASSWORD = trim(file_get_contents($credentialDirectory . '/DATASERVER_SUPER_PASSWORD'));
    $apiPath = parse_url(Z_CONFIG::$API_BASE_URI, PHP_URL_PATH);
    if ($apiPath && $apiPath !== '/') {
      $apiPrefix = rtrim($apiPath, '/');
      Z_CONFIG::$URI_PREFIX_DOMAIN_MAP = [
        '^' . preg_quote($apiPrefix, '%') => parse_url(Z_CONFIG::$API_BASE_URI, PHP_URL_HOST),
      ] + Z_CONFIG::$URI_PREFIX_DOMAIN_MAP;
    }
    ?>
  '';

  defaultDbconnect = ''
    <?
    function Zotero_DBConnectAuth($db) {
      $db = [
        "admin" => "zotero_master",
        "master" => "zotero_master",
        "shard" => "zotero_shard",
        "id1" => "zotero_ids",
        "id2" => "zotero_ids",
        "www1" => "zotero_www",
        "www2" => "zotero_www",
      ][$db];
      return [
        "host" => "localhost",
        "replicas" => [],
        "port" => ${toString config.services.mysql.settings.mysqld.port},
        "db" => $db,
        "user" => "zotero-dataserver",
        "pass" => "",
        "charset" => "utf8mb4",
        "state" => "up",
      ];
    }
    ?>
  '';

  package = cfg.package.withConfig {
    config = cfg.php.config;
    dbconnect = cfg.php.dbconnect;
  };

  schemas = {
    master = pkgs.runCommand "zotero-master.sql" {} ''
      cat ${package}/share/php/zotero-dataserver/misc/{master,coredata}.sql > $out
      cat >> $out <<'SQL'
      INSERT INTO shardHosts (shardHostID, address, port, state)
      VALUES (1, 'localhost', ${toString config.services.mysql.settings.mysqld.port}, 'up');
      INSERT INTO shards (shardID, shardHostID, db, state, items)
      VALUES (1, 1, 'zotero_shard', 'up', 0);
      SQL
    '';
    shard = pkgs.runCommand "zotero-shard.sql" {} ''
      cat ${package}/share/php/zotero-dataserver/misc/{shard,triggers}.sql > $out
    '';
  };

  cfg = config.services.zotero-dataserver;
in {
  options.services.zotero-dataserver = {
    enable = mkEnableOption "zotero-dataserver";
    package = mkPackageOption pkgs "zotero-dataserver" {};

    virtualHost = mkOption {
      type = str;
      description = "Caddy virtual host and public base URI for the Zotero services. A host name uses HTTPS; include an explicit scheme to override it.";
    };

    apiPath = mkOption {
      type = str;
      default = "/api/";
      apply = normalizePath;
      description = "URL path at which Caddy exposes the Zotero API.";
    };

    php = {
      config = mkOption {
        type = lines;
        default = defaultConfig;
        description = "Contents of the Zotero config.inc.php file.";
      };

      dbconnect = mkOption {
        type = lines;
        default = defaultDbconnect;
        description = "Contents of the Zotero dbconnect.inc.php file.";
      };

      settings = {
        apiEnabled = settingOption bool true "Whether the Zotero API accepts requests.";
        readOnly = settingOption bool false "Whether Dataserver rejects write requests.";
        maintenanceMessage = settingOption str "Server updates in progress. Please try again in a few minutes." "Message returned while the API is unavailable for maintenance.";
        backoff = settingOption int 0 "Number of seconds clients should wait before their next request.";
        testingSite = settingOption bool false "Enable Dataserver's testing-site behavior.";
        devSite = settingOption bool false "Enable development behavior on a testing site.";
        debugLog = settingOption bool false "Enable verbose Dataserver logging.";
        baseUri = settingOption str publicBaseUri "Canonical URI prefix used for Zotero objects and streaming topics.";
        reproxyMap = settingOption (attrsOf (listOf str)) {} "Legacy URI regular expressions and upstream server lists used to emit X-REPROXY-URL responses.";
        uriPrefixDomainMap = settingOption (attrsOf str) {} "URI prefixes and virtual domains used for legacy same-host API routes.";
        snsAlertTopic = settingOption str "" "Optional AWS SNS topic for operational alerts.";
        redisPrefix = settingOption str "zotero:" "Prefix shared by Dataserver and stream-server for Redis keys and notification channels.";
        globalItemsUrl = settingOption str "" "Optional URL of the global-items service.";
        ttsAudioDomain = settingOption str "" "Optional public domain from which generated TTS audio is served.";
        ttsDevUsers = settingOption (listOf int) [] "Zotero user IDs that receive text-to-speech development access.";
        errorPath = settingOption str "" "Optional directory in which Dataserver writes error reports; an empty value uses the configured S3 error bucket only.";
        searchHosts = settingOption (listOf str) ["127.0.0.1:${toString cfg.fulltext-search.settings.port}"] "OpenSearch-compatible hosts used for server-side full-text search.";

        statsd = {
          enable = settingOption bool false "Enable StatsD metrics from Dataserver.";
          prefix = settingOption str "zotero" "Prefix for StatsD metric names.";
          host = settingOption str "127.0.0.1" "StatsD server address.";
          port = settingOption port 8125 "StatsD server port.";
        };

        logToScribe = settingOption bool false "Send Dataserver application logs to a Scribe-compatible server.";
        logAddress = settingOption str "127.0.0.1" "Scribe-compatible log server address.";
        logPort = settingOption port 1463 "Scribe-compatible log server port.";
        logTimezone = settingOption str "UTC" "Timezone used in application log timestamps.";
        logTarget = settingOption str "errors" "Default remote application-log target.";
      };
    };

    storage = {
      manageBackend = settingOption bool true "Run and provision a local VersityGW S3-compatible backend.";
      endpoint = mkOption {
        type = str;
        default = "http://127.0.0.1:${toString config.services.versitygw.port}";
        apply = removeSuffix "/";
        description = "Internal S3-compatible endpoint used by Dataserver and attachment-proxy.";
      };
      region = settingOption str "us-east-1" "S3 signing region.";
      forcePathStyle = settingOption bool true "Use path-style S3 bucket URLs, as required by the managed VersityGW backend.";
      uploadBaseUrl = mkOption {
        type = str;
        default = "${cfg.storage.endpoint}/${cfg.storage.bucket}/";
        apply = normalizeUri;
        description = "Internal upload target placed in signed attachment upload parameters. The attachment proxy must be able to reach it.";
      };
      proxyAllTransfers = settingOption bool true "Proxy every client file upload and download instead of exposing the S3 endpoint.";
      bucket = settingOption str "zotero" "S3 bucket containing attachment files.";
      cacheBucket = settingOption str "zotero-cache" "S3 bucket used for generated cache objects.";
      fulltextBucket = settingOption str "zotero-fulltext" "S3 bucket containing full-text content.";
      errorsBucket = settingOption str "zotero-errors" "S3 bucket receiving archived Dataserver error reports.";
      ttsBucket = settingOption str "zotero-tts" "S3 bucket containing generated TTS audio.";
      accessKeyIdFile = settingOption (nullOr path) null "File containing the S3 access key ID. A persistent random key is generated for the managed backend when unset.";
      secretAccessKeyFile = settingOption (nullOr path) null "File containing the S3 secret access key. A persistent random key is generated for the managed backend when unset.";
    };

    fulltext-search = {
      settings = {
        port = mkOption {
          type = port;
          default = config.services.opensearch.settings."http.port";
          description = "OpenSearch HTTP port used for full-text search.";
        };
      };
      index = mkOption {
        type = submodule {
          freeformType = json.type;
          options = {
            settings = {
              number_of_shards = settingOption int 1 "Number of OpenSearch shards.";
              number_of_replicas = settingOption int 0 "Number of OpenSearch replicas.";
              refresh_interval = settingOption str "1s" "OpenSearch refresh interval.";
            };
            mappings = {
              dynamic = settingOption bool false "Whether the full-text index accepts dynamic fields.";
              _source.enabled = settingOption bool false "Whether to store the source document.";
              _routing.required = settingOption bool true "Whether index routing is required.";
              properties = {
                libraryID.type = settingOption str "integer" "OpenSearch type for library IDs.";
                content.type = settingOption str "text" "OpenSearch type for indexed content.";
              };
            };
          };
        };
        default = {};
        description = "OpenSearch index definition.";
      };
    };

    portal = {
      enable = mkOption {
        type = bool;
        default = true;
        description = "Whether to run the Zotero web portal and its database migrations.";
      };
      package = mkPackageOption pkgs "zotero-portal" {};
      clientSecretFile = mkOption {
        type = path;
        description = "File containing the OIDC client secret.";
      };
      database.urlFile = mkOption {
        type = nullOr path;
        default = null;
        description = "File containing the MariaDB connection URL. The local database is used when unset.";
      };
      config = mkOption {
        type = submodule {
          freeformType = toml.type;
          options = {
            server_address = settingOption str "unix:///run/zotero-portal/zotero-portal.sock" "Portal listen URL.";
            oidc = {
              issuer = mkOption {
                type = str;
                description = "OIDC issuer URL. The provider must expose OpenID Connect discovery metadata.";
              };
              client_id = mkOption {
                type = str;
                description = "OIDC client ID registered for the Zotero login portal.";
              };
              redirect_url = settingOption str "${removeSuffix "/" publicBaseUri}/auth/oidc/callback" "OIDC callback URL.";
            };
            dataserver_url = settingOption str publicApiBaseUri "Zotero dataserver API URL.";
            stream_server_url = settingOption str cfg.stream-server.publicUri "Streaming WebSocket URL.";
            translation_server_url = settingOption str "http://127.0.0.1:${toString cfg.translation-server.settings.port}" "Translation server URL.";
          };
        };
        default = {};
        description = "Portal TOML configuration, passed directly to the portal.";
      };
      logging = {
        filter = settingOption str "zotero_portal=info,tower_http=info" "Tracing filter for the portal, using the RUST_LOG directive syntax.";
      };
    };

    web-library.package = mkPackageOption pkgs "zotero-web-library" {};

    phpfpm.settings = mkOption {
      type = attrsOf (oneOf [str int bool]);
      default = {
        "pm" = "dynamic";
        "pm.max_children" = 8;
        "pm.start_servers" = 2;
        "pm.min_spare_servers" = 2;
        "pm.max_spare_servers" = 4;
        "pm.max_requests" = 500;
        "catch_workers_output" = true;
      };
      description = "PHP-FPM pool settings for Zotero Dataserver.";
    };

    attachment-proxy = {
      package = mkPackageOption pkgs "zotero-attachment-proxy" {};
      path = mkOption {
        type = str;
        default = "/files/";
        apply = normalizePath;
        description = "URL path at which Caddy exposes attachment-proxy.";
      };
      url = mkOption {
        type = str;
        default = "${removeSuffix "/" publicBaseUri}${cfg.attachment-proxy.path}";
        apply = normalizeUri;
        description = "Public base URI of the attachment proxy.";
      };
      settings = microserviceSettings "Attachment proxy" {
        port = settingOption port 3000 "Local attachment-proxy listen port.";
      };
    };
    citeproc-js-server = {
      package = mkPackageOption pkgs "zotero-citeproc-js-server" {};
      settings = microserviceSettings "Citation server" {
        port = settingOption port 8085 "Local citation-server listen port.";
      };
    };
    stream-server = {
      package = mkPackageOption pkgs "zotero-stream-server" {};
      path = mkOption {
        type = str;
        default = "/stream/";
        apply = normalizePath;
        description = "URL path at which Caddy exposes the streaming WebSocket service.";
      };
      publicUri = mkOption {
        type = str;
        default = "${replaceStrings ["https://" "http://"] ["wss://" "ws://"] (removeSuffix "/" publicBaseUri)}${cfg.stream-server.path}";
        apply = normalizeUri;
        description = "Public WebSocket URI to use as STREAMING_URL in a self-hosted Zotero client build.";
      };
      settings = microserviceSettings "Streaming server" {
        httpPort = settingOption port 8089 "Local streaming-server listen port.";
      };
    };
    tinymce-clean-server = {
      package = mkPackageOption pkgs "zotero-tinymce-clean-server" {};
      settings = microserviceSettings "TinyMCE cleaner" {
        log = settingOption bool true "Log TinyMCE cleaner requests.";
        port = settingOption port 16342 "Local TinyMCE cleaner listen port.";
      };
    };
    translation-server = {
      package = mkPackageOption pkgs "zotero-translation-server" {};
      settings = microserviceSettings "Translation server" {
        port = settingOption port 1969 "Local translation-server listen port.";
      };
    };
  };

  config = mkIf cfg.enable {
    services.zotero-dataserver = {
      attachment-proxy.settings.s3 = {
        bucket = mkDefault cfg.storage.bucket;
        endpoint = mkDefault cfg.storage.endpoint;
        forcePathStyle = mkDefault cfg.storage.forcePathStyle;
        region = mkDefault cfg.storage.region;
      };
      stream-server.settings = {
        apiURL = mkDefault publicApiBaseUri;
        redis = {
          url = mkDefault "redis://127.0.0.1:${toString config.services.redis.servers.zotero-dataserver.port}";
          prefix = mkDefault cfg.php.settings.redisPrefix;
        };
      };
    };

    assertions = [
      {
        assertion = (cfg.storage.accessKeyIdFile == null) == (cfg.storage.secretAccessKeyFile == null);
        message = "storage.accessKeyIdFile and storage.secretAccessKeyFile must be set together";
      }
      {
        assertion = cfg.storage.manageBackend || cfg.storage.accessKeyIdFile != null;
        message = "an external S3 backend requires storage access-key files";
      }
    ];

    users = {
      users =
        {
          zotero-dataserver = {
            isSystemUser = true;
            group = "zotero-dataserver";
          };
        }
        // optionalAttrs cfg.portal.enable {
          zotero-portal = {
            isSystemUser = true;
            group = "zotero-portal";
          };
          caddy.extraGroups = ["zotero-portal"];
        };
      groups = {zotero-dataserver = {};} // optionalAttrs cfg.portal.enable {zotero-portal = {};};
    };

    services = {
      opensearch = {
        enable = true;
        package = mkDefault minimalOpenSearch;
        settings = {
          "network.host" = mkDefault "127.0.0.1";
          "cluster.name" = mkDefault "zotero";
          "discovery.type" = mkDefault "single-node";
          "plugins.security.disabled" = mkDefault true;
        };
        extraJavaOptions = mkDefault [
          "-Xms128m"
          "-Xmx512m"
        ];
      };
      memcached.enable = true;
      redis.servers.zotero-dataserver = {
        enable = true;
        port = mkDefault 6389;
      };
      versitygw = mkIf cfg.storage.manageBackend {
        enable = true;
        rootAccessKeyIdFile = credentialFiles.s3AccessKeyId;
        rootSecretAccessKeyFile = credentialFiles.s3SecretAccessKey;
        buckets = unique [
          cfg.storage.bucket
          cfg.storage.cacheBucket
          cfg.storage.fulltextBucket
          cfg.storage.errorsBucket
          cfg.storage.ttsBucket
        ];
      };
      mysql = {
        enable = true;
        package = mkDefault pkgs.mariadb;
        ensureUsers =
          [
            {
              name = "zotero-dataserver";
              ensurePermissions = {
                "zotero_ids.*" = "ALL PRIVILEGES";
                "zotero_master.*" = "ALL PRIVILEGES";
                "zotero_shard.*" = "ALL PRIVILEGES";
                "zotero_www.*" = "ALL PRIVILEGES";
              };
            }
          ]
          ++ optional cfg.portal.enable {
            name = "zotero-portal";
            ensurePermissions = {
              "zotero_www.*" = "ALL PRIVILEGES";
            };
          };
        initialDatabases = [
          {
            name = "zotero_master";
            schema = schemas.master;
          }
          {
            name = "zotero_shard";
            schema = schemas.shard;
          }
          {
            name = "zotero_ids";
            schema = "${cfg.package}/share/php/zotero-dataserver/misc/ids.sql";
          }
          {
            name = "zotero_www";
          }
        ];
        settings.mysqld.socket = mkDefault "/run/mysqld/mysqld.sock";
      };

      phpfpm.pools.zotero-dataserver = {
        user = "zotero-dataserver";
        group = "zotero-dataserver";
        phpPackage = package.php;
        phpOptions = ''
          log_errors = on
          short_open_tag = on
        '';
        settings =
          cfg.phpfpm.settings
          // {
            "clear_env" = false;
            "listen.owner" = "zotero-dataserver";
            "listen.group" = "caddy";
            "listen.mode" = "0660";
          };
      };

      caddy = {
        enable = true;
        virtualHosts.${cfg.virtualHost} = {
          extraConfig = ''
            route {
              handle_path /static/web-library/* {
                root * ${cfg.web-library.package}/share/zotero-web-library/static
                file_server
              }

              handle_path ${removeSuffix "/" cfg.attachment-proxy.path}/* {
                reverse_proxy 127.0.0.1:${toString cfg.attachment-proxy.settings.port}
              }

              handle_path ${removeSuffix "/" cfg.stream-server.path}/* {
                reverse_proxy 127.0.0.1:${toString cfg.stream-server.settings.httpPort}
              }

              redir ${removeSuffix "/" cfg.apiPath} ${cfg.apiPath} 308
              handle ${removeSuffix "/" cfg.apiPath}/* {
                root * ${package}/share/php/zotero-dataserver/htdocs
                php_fastcgi unix/${config.services.phpfpm.pools.zotero-dataserver.socket} {
                  capture_stderr
                }
                file_server
              }

              ${optionalString cfg.portal.enable ''
              handle {
                reverse_proxy unix//run/zotero-portal/zotero-portal.sock
              }
            ''}
            }
          '';
        };
      };
    };

    environment.etc =
      optionalAttrs cfg.portal.enable {
        "zotero-dataserver/portal.toml".source = toml.generate "zotero-portal.toml" cfg.portal.config;
      }
      // {
        "zotero-dataserver/attachment-proxy".source = nodeConfigDirectory "attachment-proxy" "${cfg.attachment-proxy.package}/lib/node_modules/attachment-proxy/config" cfg.attachment-proxy.settings;
        "zotero-dataserver/citeproc-js-server".source = nodeConfigDirectory "citeproc-js-server" "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/config" cfg.citeproc-js-server.settings;
        "zotero-dataserver/stream-server".source = nodeConfigDirectory "stream-server" "${cfg.stream-server.package}/lib/node_modules/zotero-stream-server/config" cfg.stream-server.settings;
        "zotero-dataserver/translation-server".source = nodeConfigDirectory "translation-server" "${cfg.translation-server.package}/lib/node_modules/translation-server/config" cfg.translation-server.settings;
      };

    systemd.targets.zotero = {
      description = "Zotero local cloud services";
      wantedBy = ["multi-user.target"];
      wants =
        [
          provisionUnit
          "zotero-dataserver-schema.service"
          "zotero-fulltext-index.service"
          "zotero-attachment-proxy.service"
          "zotero-citeproc-js-server.service"
          "zotero-stream-server.service"
          "zotero-tinymce-clean-server.service"
          "zotero-translation-server.service"
        ]
        ++ optional cfg.portal.enable "zotero-portal.service";
    };

    systemd.services = {
      zotero-dataserver-provision = {
        description = "Provision Zotero Dataserver credentials and database";
        wants = optional cfg.portal.enable "mysql.service";
        after = optional cfg.portal.enable "mysql.service";
        environment = portalLoggingEnvironment // {XDG_STATE_HOME = "%S/zotero-dataserver";};
        serviceConfig =
          restartOnFailure
          // {
            Type = "oneshot";
            RemainAfterExit = true;
            StateDirectory = "zotero-dataserver";
            StateDirectoryMode = "0700";
            UMask = "0077";
            WorkingDirectory = "${cfg.portal.package}/share/zotero-portal";
            LoadCredential = optional cfg.portal.enable "DATABASE_URL:${portalDatabaseUrlFile}";
          }
          // optionalAttrs cfg.portal.enable {
            User = "zotero-portal";
            Group = "zotero-portal";
          };
        script = ''
          ${getExe cfg.portal.package} \
            ${optionalString cfg.portal.enable "--config /etc/zotero-dataserver/portal.toml"} \
            provision --state-directory "$STATE_DIRECTORY"
        '';
      };

      zotero-portal = mkIf cfg.portal.enable {
        description = "Zotero local web portal";
        wantedBy = ["multi-user.target"];
        wants = [
          provisionUnit
          "zotero-translation-server.service"
          "mysql.service"
          "network-online.target"
          "caddy.service"
          "phpfpm-zotero-dataserver.service"
        ];
        after = [
          provisionUnit
          "zotero-translation-server.service"
          "mysql.service"
          "network-online.target"
          "caddy.service"
          "phpfpm-zotero-dataserver.service"
        ];
        environment = portalLoggingEnvironment // {XDG_STATE_HOME = "%S/zotero-dataserver";};
        serviceConfig =
          restartAlways
          // {
            User = "zotero-portal";
            Group = "zotero-portal";
            RuntimeDirectory = "zotero-portal";
            RuntimeDirectoryMode = "0750";
            StateDirectory = "zotero-dataserver";
            StateDirectoryMode = "0700";
            UMask = "0007";
            ExecStart = "${getExe cfg.portal.package} --config /etc/zotero-dataserver/portal.toml serve";
            WorkingDirectory = "${cfg.portal.package}/share/zotero-portal";
            LoadCredential = [
              "DATABASE_URL:${portalDatabaseUrlFile}"
              "AUTH_SALT:${credentialFiles.authSalt}"
              "ATTACHMENT_PROXY_SECRET:${credentialFiles.attachmentProxySecret}"
              "OIDC_CLIENT_SECRET:${cfg.portal.clientSecretFile}"
              "DATASERVER_SUPER_PASSWORD:${credentialFiles.dataserverSuperPassword}"
              "PORTAL_COOKIE_KEY:${credentialFiles.portalCookieKey}"
              "S3_ACCESS_KEY_ID:${credentialFiles.s3AccessKeyId}"
              "S3_SECRET_ACCESS_KEY:${credentialFiles.s3SecretAccessKey}"
            ];
          };
      };

      phpfpm-zotero-dataserver = {
        wants =
          [
            provisionUnit
            "zotero-dataserver-schema.service"
            "zotero-attachment-proxy.service"
            "zotero-citeproc-js-server.service"
            "zotero-stream-server.service"
            "zotero-tinymce-clean-server.service"
            "zotero-translation-server.service"
            "mysql.service"
            "redis-zotero-dataserver.service"
            "memcached.service"
          ]
          ++ optional cfg.storage.manageBackend "versitygw.service";
        requires = ["zotero-fulltext-index.service" "opensearch.service"];
        after =
          [
            provisionUnit
            "zotero-dataserver-schema.service"
            "zotero-fulltext-index.service"
            "zotero-attachment-proxy.service"
            "zotero-citeproc-js-server.service"
            "zotero-stream-server.service"
            "zotero-tinymce-clean-server.service"
            "zotero-translation-server.service"
            "mysql.service"
            "redis-zotero-dataserver.service"
            "memcached.service"
            "opensearch.service"
          ]
          ++ optional cfg.storage.manageBackend "versitygw.service";
        serviceConfig = {
          User = "zotero-dataserver";
          Group = "zotero-dataserver";
          SupplementaryGroups = ["caddy"];
          LoadCredential = [
            "AUTH_SALT:${credentialFiles.authSalt}"
            "ATTACHMENT_PROXY_SECRET:${credentialFiles.attachmentProxySecret}"
            "DATASERVER_SUPER_PASSWORD:${credentialFiles.dataserverSuperPassword}"
            "S3_ACCESS_KEY_ID:${credentialFiles.s3AccessKeyId}"
            "S3_SECRET_ACCESS_KEY:${credentialFiles.s3SecretAccessKey}"
          ];
        };
      };

      zotero-dataserver-schema = {
        description = "Update Zotero Dataserver's metadata schema";
        wants = [provisionUnit "mysql.service" "redis-zotero-dataserver.service" "memcached.service"];
        after = [provisionUnit "mysql.service" "redis-zotero-dataserver.service" "memcached.service"];
        serviceConfig =
          restartOnFailure
          // {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "zotero-dataserver";
            Group = "zotero-dataserver";
            WorkingDirectory = "${package}/share/php/zotero-dataserver/admin";
            ExecStart = "${getExe package.php} ${package}/share/php/zotero-dataserver/admin/schema_update";
            LoadCredential = [
              "AUTH_SALT:${credentialFiles.authSalt}"
              "ATTACHMENT_PROXY_SECRET:${credentialFiles.attachmentProxySecret}"
              "DATASERVER_SUPER_PASSWORD:${credentialFiles.dataserverSuperPassword}"
              "S3_ACCESS_KEY_ID:${credentialFiles.s3AccessKeyId}"
              "S3_SECRET_ACCESS_KEY:${credentialFiles.s3SecretAccessKey}"
            ];
          };
      };

      zotero-fulltext-index = {
        description = "Initialize Zotero's local full-text search index";
        requires = ["opensearch.service"];
        wants = [provisionUnit] ++ optional cfg.storage.manageBackend "versitygw.service";
        after = [provisionUnit "opensearch.service"] ++ optional cfg.storage.manageBackend "versitygw.service";
        before = ["phpfpm-zotero-dataserver.service"];
        serviceConfig =
          restartOnFailure
          // {
            Type = "oneshot";
            RemainAfterExit = true;
            User = "zotero-dataserver";
            Group = "zotero-dataserver";
            StateDirectory = "zotero-fulltext";
            StateDirectoryMode = "0700";
            WorkingDirectory = "${package}/share/php/zotero-dataserver/admin";
            LoadCredential = [
              "AUTH_SALT:${credentialFiles.authSalt}"
              "ATTACHMENT_PROXY_SECRET:${credentialFiles.attachmentProxySecret}"
              "DATASERVER_SUPER_PASSWORD:${credentialFiles.dataserverSuperPassword}"
              "S3_ACCESS_KEY_ID:${credentialFiles.s3AccessKeyId}"
              "S3_SECRET_ACCESS_KEY:${credentialFiles.s3SecretAccessKey}"
            ];
          };
        script = ''
          index_created=
          if ! ${getExe pkgs.curl} --fail --silent --output /dev/null ${fulltextIndexUrl}; then
            ${getExe pkgs.curl} --fail-with-body --silent \
              --request PUT \
              --header 'Content-Type: application/json' \
              --data-binary @${fulltextIndex} \
              '${fulltextIndexUrl}'
            index_created=1
          fi

          index_marker="$STATE_DIRECTORY/index-v1"
          if test -n "$index_created" || ! test -e "$index_marker"; then
            ${getExe package.php} ${package}/share/php/zotero-dataserver/admin/reindex_fulltext
            touch "$index_marker"
          fi
        '';
      };

      versitygw = mkIf cfg.storage.manageBackend {
        wants = [provisionUnit];
        after = [provisionUnit];
      };

      zotero-attachment-proxy = {
        description = "Zotero attachment-proxy";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        wants = [provisionUnit] ++ optional cfg.storage.manageBackend "versitygw.service";
        after = [provisionUnit] ++ optional cfg.storage.manageBackend "versitygw.service";
        environment = {
          NODE_ENV = "attachment-proxy";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver/attachment-proxy";
        };
        script = ''
          export AWS_ACCESS_KEY_ID="$(< "$CREDENTIALS_DIRECTORY/S3_ACCESS_KEY_ID")"
          export AWS_SECRET_ACCESS_KEY="$(< "$CREDENTIALS_DIRECTORY/S3_SECRET_ACCESS_KEY")"
          exec ${getExe cfg.attachment-proxy.package}
        '';
        serviceConfig = {
          LoadCredential = [
            "ATTACHMENT_PROXY_SECRET:${credentialFiles.attachmentProxySecret}"
            "S3_ACCESS_KEY_ID:${credentialFiles.s3AccessKeyId}"
            "S3_SECRET_ACCESS_KEY:${credentialFiles.s3SecretAccessKey}"
          ];
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
          CacheDirectory = "zotero-attachment-proxy";
        };
      };
      zotero-citeproc-js-server = {
        description = "Zotero citeproc-js-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        environment = {
          NODE_ENV = "citeproc-js-server";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver/citeproc-js-server";
        };
        serviceConfig = {
          ExecStart = "${getExe cfg.citeproc-js-server.package}";
          WorkingDirectory = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server";
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
        };
      };
      zotero-stream-server = {
        description = "Zotero stream-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        wants = ["redis-zotero-dataserver.service"];
        after = ["redis-zotero-dataserver.service"];
        environment = {
          NODE_ENV = "stream-server";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver/stream-server";
        };
        serviceConfig = {
          ExecStart = "${getExe cfg.stream-server.package}";
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
        };
      };
      zotero-tinymce-clean-server = {
        description = "Zotero tinymce-clean-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        environment = let
          cfg' = cfg.tinymce-clean-server.settings;
        in
          {
            TINYMCE_CLEAN_PORT = toString cfg'.port;
            TINYMCE_CLEAN_HOST = "127.0.0.1";
          }
          // optionalAttrs cfg'.log {TINYMCE_CLEAN_LOG = "1";};
        serviceConfig = {
          ExecStart = "${getExe cfg.tinymce-clean-server.package}";
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
        };
      };
      zotero-translation-server = {
        description = "Zotero translation-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        wants = ["network-online.target"];
        after = ["network-online.target"];
        environment = {
          NODE_ENV = "translation-server";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver/translation-server";
        };
        serviceConfig = {
          ExecStart = "${getExe cfg.translation-server.package}";
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
        };
      };
    };
  };

  _class = "nixos";
  _file = ./zotero-dataserver.nix;
}
