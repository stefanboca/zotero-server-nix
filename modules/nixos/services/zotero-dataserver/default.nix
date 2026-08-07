{
  config,
  lib,
  pkgs,
  ...
}: let
  inherit (lib.attrsets) mapAttrsToList optionalAttrs;
  inherit (lib.meta) getExe;
  inherit (lib.lists) optional unique;
  inherit (lib.modules) mkDefault mkIf;
  inherit (lib.options) mkEnableOption mkOption mkPackageOption;
  inherit (lib.strings) concatMapStrings concatStringsSep hasPrefix optionalString removePrefix removeSuffix replaceStrings;
  inherit (lib.types) attrsOf bool int lines listOf oneOf path port str nullOr;

  json = pkgs.formats.json {};
  toml = pkgs.formats.toml {};

  redisPort = toString config.services.redis.servers.zotero-dataserver.port;

  credentialUnit = "zotero-dataserver-credentials.service";
  generatedCredentialDir = "%S/zotero-dataserver/credentials";

  credentialFiles = {
    authSalt = "${generatedCredentialDir}/auth-salt";
    attachmentProxySecret = "${generatedCredentialDir}/attachment-proxy-secret";
    dataserverSuperPassword = "${generatedCredentialDir}/dataserver-super-password";
    s3AccessKeyId =
      if cfg.storage.accessKeyIdFile != null
      then cfg.storage.accessKeyIdFile
      else "${generatedCredentialDir}/s3-access-key-id";
    s3SecretAccessKey =
      if cfg.storage.secretAccessKeyFile != null
      then cfg.storage.secretAccessKeyFile
      else "${generatedCredentialDir}/s3-secret-access-key";
  };

  generatedCredentials = [
    {
      generate = true;
      name = "auth-salt";
      bytes = 32;
    }
    {
      generate = true;
      name = "attachment-proxy-secret";
      bytes = 32;
    }
    {
      generate = true;
      name = "dataserver-super-password";
      bytes = 32;
    }
    {
      generate = cfg.storage.accessKeyIdFile == null;
      name = "s3-access-key-id";
      bytes = 16;
    }
    {
      generate = cfg.storage.secretAccessKeyFile == null;
      name = "s3-secret-access-key";
      bytes = 32;
    }
  ];

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

  normalizeUri = uri: "${removeSuffix "/" uri}/";
  normalizePath = path: "/${removePrefix "/" (removeSuffix "/" path)}/";
  virtualHostUri =
    if hasPrefix "http://" cfg.virtualHost || hasPrefix "https://" cfg.virtualHost
    then cfg.virtualHost
    else "https://${cfg.virtualHost}";

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
      public static $API_BASE_URI = ${phpString cfg.apiBaseUri};
      public static $WWW_BASE_URI = ${phpString cfg.wwwBaseUri};
      public static $REPROXY_MAP = ${phpStringListAttrs cfg.php.settings.reproxyMap};
      public static $URI_PREFIX_DOMAIN_MAP = ${phpStringAttrs cfg.php.settings.uriPrefixDomainMap};

      public static $AUTH_SALT = "";
      public static $API_SUPER_USERNAME = 'zotero-auth';
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
      public static $FULLTEXT_INDEXING_TABLE = ${phpString cfg.php.settings.fulltextIndexingTable};
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

      public static $SEARCH_HOSTS = ${builtins.toJSON cfg.php.settings.searchHosts};

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
      public static $TTS_DEV_USERS = ${builtins.toJSON cfg.php.settings.ttsDevUsers};

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

      public static $REINDEX_QUEUE_URL = ${phpString cfg.php.settings.reindexQueueUrl};
    }

    $credentialDirectory = getenv('CREDENTIALS_DIRECTORY');
    Z_CONFIG::$AUTH_SALT = trim(file_get_contents($credentialDirectory . '/auth-salt'));
    Z_CONFIG::$ATTACHMENT_PROXY_SECRET = trim(file_get_contents($credentialDirectory . '/attachment-proxy-secret'));
    Z_CONFIG::$AWS_ACCESS_KEY = trim(file_get_contents($credentialDirectory . '/s3-access-key-id'));
    Z_CONFIG::$AWS_SECRET_KEY = trim(file_get_contents($credentialDirectory . '/s3-secret-access-key'));
    Z_CONFIG::$API_SUPER_PASSWORD = trim(file_get_contents($credentialDirectory . '/dataserver-super-password'));
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
        "charset" => "",
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
      description = "Caddy virtual host for the Zotero API, file proxy, stream service, and login portal.";
    };

    apiBaseUri = mkOption {
      type = str;
      default = virtualHostUri;
      apply = normalizeUri;
      description = "Public base URI of the Zotero API.";
    };

    wwwBaseUri = mkOption {
      type = str;
      default = cfg.apiBaseUri;
      apply = normalizeUri;
      description = "Public base URI of the Zotero website.";
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
        baseUri = settingOption str cfg.apiBaseUri "Canonical URI prefix used for Zotero objects and streaming topics.";
        reproxyMap = settingOption (attrsOf (listOf str)) {} "Legacy URI regular expressions and upstream server lists used to emit X-REPROXY-URL responses.";
        uriPrefixDomainMap = settingOption (attrsOf str) {} "URI prefixes and virtual domains used for legacy same-host API routes.";
        snsAlertTopic = settingOption str "" "Optional AWS SNS topic for operational alerts.";
        redisPrefix = settingOption str "zotero:" "Prefix shared by Dataserver and stream-server for Redis keys and notification channels.";
        globalItemsUrl = settingOption str "" "Optional URL of the global-items service.";
        ttsAudioDomain = settingOption str "" "Optional public domain from which generated TTS audio is served.";
        ttsDevUsers = settingOption (listOf int) [] "Zotero user IDs that receive text-to-speech development access.";
        errorPath = settingOption str "" "Optional directory in which Dataserver writes error reports; an empty value uses the configured S3 error bucket only.";
        searchHosts = settingOption (listOf str) [] "Elasticsearch hosts used for server-side metadata and full-text search. An empty list disables external search.";
        fulltextIndexingTable = settingOption str "FullTextIndexing" "DynamoDB table shared with Zotero's optional external full-text indexer.";
        reindexQueueUrl = settingOption str "" "SQS queue URL used by Zotero's optional external full-text reindexer. Leave this empty when external indexing is not deployed.";

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

    auth = {
      enable = mkEnableOption "the OIDC-only Zotero desktop login portal";
      package = mkPackageOption pkgs "zotero-auth" {};
      issuer = mkOption {
        type = nullOr str;
        default = null;
        description = "OIDC issuer URL. The provider must expose OpenID Connect discovery metadata.";
      };
      clientId = mkOption {
        type = nullOr str;
        default = null;
        description = "OIDC client ID registered for the Zotero login portal.";
      };
      clientSecretFile = mkOption {
        type = nullOr path;
        default = null;
        description = "File containing the OIDC client secret.";
      };
      redirectUri = mkOption {
        type = str;
        default = "${cfg.wwwBaseUri}auth/callback";
        apply = removeSuffix "/";
        description = "OIDC callback URI, which must exactly match the provider's client registration.";
      };
      scopes = settingOption (listOf str) ["profile" "email"] "OIDC scopes requested in addition to the mandatory openid scope.";
      usernameClaim = settingOption str "preferred_username" "OIDC claim used as the initial Zotero username; dotted paths address nested claims.";
      realNameClaim = settingOption str "name" "OIDC claim synchronized to the Zotero profile real name; dotted paths address nested claims.";
      requireVerifiedEmail = settingOption bool true "Reject identities whose OIDC email_verified claim is not true.";
      databaseMaxConnections = settingOption int 5 "Maximum MariaDB connections used by the login portal.";
      internalApiPort = settingOption port 16213 "Loopback-only Caddy port used by the login portal to complete Dataserver login sessions.";
    };

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
        default = "${removeSuffix "/" cfg.apiBaseUri}${cfg.attachment-proxy.path}";
        apply = normalizeUri;
        description = "Public base URI of the attachment proxy.";
      };
      settings = {
        logLevel = mkOption {
          type = str;
          default = "info";
          description = "Attachment proxy log level.";
        };
        logFile = mkOption {
          type = str;
          default = "";
          description = "Attachment proxy log file, or standard output when empty.";
        };
        port = settingOption port 3000 "Local attachment-proxy listen port.";
        zipCacheTime = mkOption {
          type = int;
          default = 60;
          description = "Seconds to retain a downloaded ZIP in the attachment-proxy cache.";
        };
        zipMaxFiles = mkOption {
          type = int;
          default = 1000;
          description = "Maximum number of entries accepted in an attachment ZIP; zero disables the limit.";
        };
        zipMaxFileSize = mkOption {
          type = int;
          default = 128 * 1024 * 1024;
          description = "Maximum uncompressed ZIP entry size in bytes; zero disables the limit.";
        };
        connectionTimeout = mkOption {
          type = int;
          default = 30;
          description = "Client connection inactivity timeout in seconds.";
        };
      };
    };
    citeproc-js-server = {
      package = mkPackageOption pkgs "zotero-citeproc-js-server" {};
      settings = {
        port = settingOption port 8085 "Local citation-server listen port.";
        logLevel = settingOption str "info" "Citation-server log level.";
        engineCacheSize = settingOption int 20 "Maximum number of cached CSL engines.";
        timings = settingOption bool false "Log citation processing timings.";
        debug = settingOption bool false "Enable citation-server debugging endpoints and output.";
        allowCors = settingOption bool false "Allow cross-origin requests directly to citation-server.";
        userAgent = settingOption str "Zotero Citation Server (https://github.com/zotero/citeproc-js-server)" "User-Agent used when citation-server fetches styles.";
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
        default = "${replaceStrings ["https://" "http://"] ["wss://" "ws://"] (removeSuffix "/" cfg.apiBaseUri)}${cfg.stream-server.path}";
        apply = normalizeUri;
        description = "Public WebSocket URI to use as STREAMING_URL in a self-hosted Zotero client build.";
      };
      settings = {
        dev = settingOption bool false "Expose detailed server errors intended for development.";
        logLevel = settingOption str "info" "Streaming-server log level.";
        httpPort = settingOption port 8080 "Local streaming-server listen port.";
        statusInterval = settingOption int 10 "Seconds between streaming-server status reports.";
        keepaliveInterval = settingOption int 25 "Seconds between WebSocket keepalive pings.";
        retryTime = settingOption int 10 "Seconds clients are asked to wait before reconnecting.";
        globalTopics = settingOption (listOf str) ["styles" "translators"] "Public topics clients may subscribe to without an API key.";
        globalTopicsMinDelay = settingOption int (60 * 1000) "Minimum client delay in milliseconds for global-topic notifications.";
        globalTopicsDelayPeriod = settingOption int (30 * 60 * 1000) "Randomized client-delay window in milliseconds for global-topic notifications.";
        defaultDelay = settingOption int (3 * 1000) "Default client notification delay in milliseconds.";
        continuedDelay = settingOption int (30 * 1000) "Client delay in milliseconds for continued notifications.";
        notContinuedDelay = settingOption int 250 "Client delay in milliseconds when a continued update ends.";
        rateLimitRequests = settingOption int 10 "Maximum key-validation requests per client during a rate-limit window.";
        rateLimitWindow = settingOption int 60 "Streaming key-validation rate-limit window in seconds.";
        statsD.host = settingOption str "" "Optional StatsD host for streaming-server metrics; empty disables metrics.";
      };
    };
    tinymce-clean-server = {
      package = mkPackageOption pkgs "zotero-tinymce-clean-server" {};
      settings = {
        log = settingOption bool true "Log TinyMCE cleaner requests.";
        port = settingOption port 16342 "Local TinyMCE cleaner listen port.";
      };
    };
    translation-server = {
      package = mkPackageOption pkgs "zotero-translation-server" {};
      settings = {
        blacklistedDomains = settingOption (listOf str) [] "Domains translation-server must not fetch.";
        deproxifyURLs = settingOption bool false "Try deproxified versions of proxied URLs during web translation.";
        identifierSearchLambda = settingOption str "" "Optional AWS Lambda function used for free-form identifier searches.";
        port = settingOption port 1969 "Local translation-server listen port.";
        httpMaxResponseSize = settingOption int 10 "Maximum fetched response size in megabytes.";
        textSearchTimeout = settingOption int 5 "Free-form identifier-search timeout in seconds.";
        userAgent = settingOption str "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/110.0.0.0 Safari/537.36" "User-Agent used for translation-server web requests.";
        crossrefEmail = settingOption str "" "Optional contact email sent to Crossref REST to use its polite pool.";
      };
    };
  };

  config = mkIf cfg.enable {
    assertions = [
      {
        assertion = (cfg.storage.accessKeyIdFile == null) == (cfg.storage.secretAccessKeyFile == null);
        message = "storage.accessKeyIdFile and storage.secretAccessKeyFile must be set together";
      }
      {
        assertion = cfg.storage.manageBackend || cfg.storage.accessKeyIdFile != null;
        message = "an external S3 backend requires storage access-key files";
      }
      {
        assertion = !cfg.auth.enable || cfg.auth.issuer != null;
        message = "services.zotero-dataserver.auth.issuer must be set when OIDC login is enabled";
      }
      {
        assertion = !cfg.auth.enable || cfg.auth.clientId != null;
        message = "services.zotero-dataserver.auth.clientId must be set when OIDC login is enabled";
      }
      {
        assertion = !cfg.auth.enable || cfg.auth.clientSecretFile != null;
        message = "services.zotero-dataserver.auth.clientSecretFile must be set when OIDC login is enabled";
      }
    ];

    users.groups.zotero-dataserver = {};
    users.users.zotero-dataserver = {
      isSystemUser = true;
      group = "zotero-dataserver";
    };
    users.groups.zotero-auth = {};
    users.users.zotero-auth = {
      isSystemUser = true;
      group = "zotero-auth";
    };
    users.users.caddy.extraGroups = ["zotero-auth"];

    services = {
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
        ensureUsers = [
          {
            name = "zotero-dataserver";
            ensurePermissions = {
              "zotero_ids.*" = "ALL PRIVILEGES";
              "zotero_master.*" = "ALL PRIVILEGES";
              "zotero_shard.*" = "ALL PRIVILEGES";
              "zotero_www.*" = "ALL PRIVILEGES";
            };
          }
          {
            name = "zotero-auth";
            ensurePermissions = {
              "zotero_www.*" = "ALL PRIVILEGES";
            };
          }
        ];
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
        settings.mysqld = {
          # Dataserver uses InnoDB, except for its MyISAM ID allocator. MariaDB
          # also uses Aria internally, so retain those engines with small caches.
          aria_pagecache_buffer_size = mkDefault "8M";
          aria_sort_buffer_size = mkDefault "16M";
          bind-address = mkDefault "127.0.0.1";
          host_cache_size = mkDefault 0;
          innodb_buffer_pool_size = mkDefault "64M";
          innodb_ft_cache_size = mkDefault "2M";
          innodb_ft_total_cache_size = mkDefault "32M";
          innodb_log_buffer_size = mkDefault "8M";
          innodb_read_io_threads = mkDefault 2;
          innodb_write_io_threads = mkDefault 2;
          key_buffer_size = mkDefault "8M";
          max_connections = mkDefault 32;
          max_heap_table_size = mkDefault "8M";
          myisam_sort_buffer_size = mkDefault "16M";
          performance_schema = mkDefault "OFF";
          port = mkDefault 3336;
          query_cache_size = mkDefault 0;
          sequence = mkDefault "OFF";
          skip_name_resolve = mkDefault true;
          socket = mkDefault "/run/mysqld/mysqld.sock";
          table_open_cache = mkDefault 400;
          table_open_cache_instances = mkDefault 1;
          thread_cache_size = mkDefault 8;
          tmp_table_size = mkDefault "8M";
        };
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
            ${optionalString cfg.auth.enable ''
              handle /login {
                reverse_proxy unix//run/zotero-auth/zotero-auth.sock
              }

              handle /auth/* {
                reverse_proxy unix//run/zotero-auth/zotero-auth.sock
              }
            ''}

            handle_path ${removeSuffix "/" cfg.attachment-proxy.path}/* {
              reverse_proxy 127.0.0.1:${toString cfg.attachment-proxy.settings.port}
            }

            handle_path ${removeSuffix "/" cfg.stream-server.path}/* {
              reverse_proxy 127.0.0.1:${toString cfg.stream-server.settings.httpPort}
            }

            root * ${package}/share/php/zotero-dataserver/htdocs
            php_fastcgi unix/${config.services.phpfpm.pools.zotero-dataserver.socket} {
              capture_stderr
            }
            file_server
          '';
        };
        virtualHosts."http://127.0.0.1:${toString cfg.auth.internalApiPort}" = mkIf cfg.auth.enable {
          extraConfig = ''
            handle /keys/sessions/* {
              root * ${package}/share/php/zotero-dataserver/htdocs
              php_fastcgi unix/${config.services.phpfpm.pools.zotero-dataserver.socket} {
                capture_stderr
              }
            }

            respond 404
          '';
        };
      };
    };

    environment.etc = {
      "zotero-dataserver/auth.toml".source = toml.generate "zotero-auth.toml" {
        server = {
          socket = "/run/zotero-auth/zotero-auth.sock";
        };
        database = {
          socket = toString config.services.mysql.settings.mysqld.socket;
          user = "zotero-auth";
          name = "zotero_www";
          max_connections = cfg.auth.databaseMaxConnections;
        };
        oidc = {
          issuer =
            if cfg.auth.issuer == null
            then ""
            else cfg.auth.issuer;
          client_id =
            if cfg.auth.clientId == null
            then ""
            else cfg.auth.clientId;
          redirect_uri = cfg.auth.redirectUri;
          scopes = cfg.auth.scopes;
        };
        identity = {
          username_claim = cfg.auth.usernameClaim;
          real_name_claim = cfg.auth.realNameClaim;
          require_verified_email = cfg.auth.requireVerifiedEmail;
        };
        dataserver.url = "http://127.0.0.1:${toString cfg.auth.internalApiPort}";
      };
      "zotero-dataserver/attachment-proxy.json".source = json.generate "attachment-proxy.json" (cfg.attachment-proxy.settings
        // {
          host = "127.0.0.1";
          tmpDir = "";
          trustedProxies = ["127.0.0.1"];
          s3 = {
            bucket = cfg.storage.bucket;
            endpoint = cfg.storage.endpoint;
            forcePathStyle = cfg.storage.forcePathStyle;
            region = cfg.storage.region;
          };
        });
      "zotero-dataserver/citeproc-js-server.json".source = json.generate "citeproc-js-server-config.json" (cfg.citeproc-js-server.settings
        // {
          host = "127.0.0.1";
          healthCheckFile = "";
          localesPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl-locales";
          cslPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl";
          renamedStylesPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl/renamed-styles.json";
          cslDependentPath = "${cfg.citeproc-js-server.package}/lib/node_modules/citeproc-js-server/csl/dependent";
        });
      "zotero-dataserver/stream-server.json".source = json.generate "stream-server-config.json" (cfg.stream-server.settings
        // {
          hostname = config.networking.hostName;
          listenAddress = "127.0.0.1";
          proxyProtocol = false;
          https = false;
          trustedProxies = ["127.0.0.1"];
          apiURL = cfg.apiBaseUri;
          apiVersion = 3;
          apiRequestHeaders = {};
          redis = {
            url = "redis://127.0.0.1:${toString config.services.redis.servers.zotero-dataserver.port}";
            prefix = cfg.php.settings.redisPrefix;
          };
        });
      "zotero-dataserver/translation-server.json".source = json.generate "translation-server-config.json" ((removeAttrs cfg.translation-server.settings ["crossrefEmail"])
        // {
          allowedOrigins = [];
          host = "127.0.0.1";
          trustProxyHeaders = false;
          translators = {
            "CrossrefREST.email" = cfg.translation-server.settings.crossrefEmail;
          };
          translatorsDirectory = "./modules/translators";
        });
    };

    systemd.services = {
      zotero-dataserver-credentials = {
        description = "Generate persistent Zotero Dataserver credentials";
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          StateDirectory = "zotero-dataserver";
          StateDirectoryMode = "0700";
          UMask = "0077";
        };
        script = ''
          install -d -m 0700 "$STATE_DIRECTORY/credentials"
          ${concatMapStrings (credential:
            optionalString credential.generate ''
              if ! test -s "$STATE_DIRECTORY/credentials/${credential.name}"; then
                ${getExe pkgs.openssl} rand -hex ${toString credential.bytes} > "$STATE_DIRECTORY/credentials/${credential.name}"
              fi
            '')
          generatedCredentials}
        '';
      };

      zotero-auth-migrate = {
        description = "Migrate the Zotero website compatibility database";
        requires = ["mysql.service"];
        after = ["mysql.service"];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          User = "zotero-auth";
          Group = "zotero-auth";
          ExecStart = "${getExe cfg.auth.package} --config /etc/zotero-dataserver/auth.toml --migrate-only";
        };
      };

      zotero-auth = mkIf cfg.auth.enable {
        description = "OIDC login portal for Zotero Dataserver";
        wantedBy = ["multi-user.target"];
        requires = [credentialUnit "zotero-auth-migrate.service"];
        wants = ["network-online.target" "caddy.service" "phpfpm-zotero-dataserver.service"];
        after = [credentialUnit "zotero-auth-migrate.service" "network-online.target" "caddy.service" "phpfpm-zotero-dataserver.service"];
        serviceConfig = {
          User = "zotero-auth";
          Group = "zotero-auth";
          RuntimeDirectory = "zotero-auth";
          RuntimeDirectoryMode = "0750";
          UMask = "0007";
          ExecStart = "${getExe cfg.auth.package} --config /etc/zotero-dataserver/auth.toml";
          Restart = "always";
          RestartSec = 5;
          LoadCredential = [
            "oidc-client-secret:${
              if cfg.auth.clientSecretFile == null
              then "/dev/null"
              else cfg.auth.clientSecretFile
            }"
            "dataserver-super-password:${credentialFiles.dataserverSuperPassword}"
          ];
        };
      };

      phpfpm-zotero-dataserver = {
        requires = ["zotero-dataserver-schema.service"];
        wants = [
          "zotero-attachment-proxy.service"
          "zotero-citeproc-js-server.service"
          "zotero-stream-server.service"
          "zotero-tinymce-clean-server.service"
          "zotero-translation-server.service"
        ];
        after = ["zotero-dataserver-schema.service"];
        serviceConfig = {
          User = "zotero-dataserver";
          Group = "zotero-dataserver";
          SupplementaryGroups = ["caddy"];
          LoadCredential = [
            "auth-salt:${credentialFiles.authSalt}"
            "attachment-proxy-secret:${credentialFiles.attachmentProxySecret}"
            "dataserver-super-password:${credentialFiles.dataserverSuperPassword}"
            "s3-access-key-id:${credentialFiles.s3AccessKeyId}"
            "s3-secret-access-key:${credentialFiles.s3SecretAccessKey}"
          ];
        };
      };

      zotero-dataserver-schema = {
        description = "Update Zotero Dataserver's metadata schema";
        requires = [credentialUnit "mysql.service" "redis-zotero-dataserver.service" "memcached.service" "zotero-auth-migrate.service"];
        after = [credentialUnit "mysql.service" "redis-zotero-dataserver.service" "memcached.service" "zotero-auth-migrate.service"];
        serviceConfig = {
          Type = "oneshot";
          RemainAfterExit = true;
          User = "zotero-dataserver";
          Group = "zotero-dataserver";
          WorkingDirectory = "${package}/share/php/zotero-dataserver/admin";
          ExecStart = "${getExe package.php} ${package}/share/php/zotero-dataserver/admin/schema_update";
          LoadCredential = [
            "auth-salt:${credentialFiles.authSalt}"
            "attachment-proxy-secret:${credentialFiles.attachmentProxySecret}"
            "dataserver-super-password:${credentialFiles.dataserverSuperPassword}"
            "s3-access-key-id:${credentialFiles.s3AccessKeyId}"
            "s3-secret-access-key:${credentialFiles.s3SecretAccessKey}"
          ];
        };
      };

      versitygw = mkIf cfg.storage.manageBackend {
        requires = [credentialUnit];
        after = [credentialUnit];
      };

      zotero-attachment-proxy = {
        description = "Zotero attachment-proxy";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        requires = [credentialUnit] ++ optional cfg.storage.manageBackend "versitygw.service";
        after = [credentialUnit] ++ optional cfg.storage.manageBackend "versitygw.service";
        environment = {
          NODE_ENV = "attachment-proxy";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver";
        };
        script = ''
          export AWS_ACCESS_KEY_ID="$(< "$CREDENTIALS_DIRECTORY/s3-access-key-id")"
          export AWS_SECRET_ACCESS_KEY="$(< "$CREDENTIALS_DIRECTORY/s3-secret-access-key")"
          exec ${getExe cfg.attachment-proxy.package}
        '';
        serviceConfig = {
          LoadCredential = [
            "attachment-proxy-secret:${credentialFiles.attachmentProxySecret}"
            "s3-access-key-id:${credentialFiles.s3AccessKeyId}"
            "s3-secret-access-key:${credentialFiles.s3SecretAccessKey}"
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
          NODE_CONFIG_DIR = "/etc/zotero-dataserver";
        };
        serviceConfig = {
          ExecStart = "${getExe cfg.citeproc-js-server.package}";
          Restart = "always";
          RestartSec = 5;
          DynamicUser = true;
        };
      };
      zotero-stream-server = {
        description = "Zotero stream-server";
        wantedBy = ["phpfpm-zotero-dataserver.service"];
        requires = ["redis-zotero-dataserver.service"];
        after = ["redis-zotero-dataserver.service"];
        environment = {
          NODE_ENV = "stream-server";
          NODE_CONFIG_DIR = "/etc/zotero-dataserver";
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
          NODE_CONFIG_DIR = "/etc/zotero-dataserver";
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
