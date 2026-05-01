use clap::Parser;

/// Static file serving and directory listing
#[derive(Parser, Debug, Clone)]
#[command(
    name = "vaserve",
    about = "Static file serving and directory listing",
    disable_help_flag = true,
    disable_version_flag = true,
    after_help = "ENDPOINTS\n\n\
        Listen endpoints (specified by the --listen or -l options above) instruct vaserve\n\
        to listen on one or more interfaces/ports, UNIX domain sockets, or Windows named pipes.\n\
        \n\
        For TCP ports on hostname \"localhost\":\n\n          $ vaserve -l 1234\n\
        \n\
        For TCP (traditional host/port) endpoints:\n\n          $ vaserve -l tcp://hostname:1234\n\
        \n\
        For UNIX domain socket endpoints:\n\n          $ vaserve -l unix:/path/to/socket.sock\n\
        \n\
        For Windows named pipe endpoints:\n\n          $ vaserve -l pipe:\\\\.\\pipe\\PipeName"
)]
pub struct CliArgs {
    /// Show help message
    #[arg(short = 'h', long = "help", action = clap::ArgAction::SetTrue)]
    pub help: bool,

    /// Display version
    #[arg(short = 'v', long = "version", action = clap::ArgAction::SetTrue)]
    pub version: bool,

    /// Specify a URI endpoint on which to listen
    #[arg(short = 'l', long = "listen", value_name = "listen_uri")]
    pub listen: Vec<String>,

    /// Specify custom port (deprecated, use --listen)
    #[arg(short = 'p', value_name = "port")]
    pub port: Option<u16>,

    /// Rewrite all not-found requests to `index.html`
    #[arg(short = 's', long = "single", action = clap::ArgAction::SetTrue)]
    pub single: bool,

    /// Show debugging information
    #[arg(short = 'd', long = "debug", action = clap::ArgAction::SetTrue)]
    pub debug: bool,

    /// Specify custom path to `serve.json`
    #[arg(short = 'c', long = "config", value_name = "path")]
    pub config: Option<String>,

    /// Do not log any request information to the console
    #[arg(short = 'L', long = "no-request-logging", action = clap::ArgAction::SetTrue)]
    pub no_request_logging: bool,

    /// Enable CORS, sets `Access-Control-Allow-Origin` to `*`
    #[arg(short = 'C', long = "cors", action = clap::ArgAction::SetTrue)]
    pub cors: bool,

    /// Do not copy the local address to the clipboard
    #[arg(short = 'n', long = "no-clipboard", action = clap::ArgAction::SetTrue)]
    pub no_clipboard: bool,

    /// Do not compress files
    #[arg(short = 'u', long = "no-compression", action = clap::ArgAction::SetTrue)]
    pub no_compression: bool,

    /// Send `Last-Modified` header instead of `ETag`
    #[arg(long = "no-etag", action = clap::ArgAction::SetTrue)]
    pub no_etag: bool,

    /// Resolve symlinks instead of showing 404 errors
    #[arg(short = 'S', long = "symlinks", action = clap::ArgAction::SetTrue)]
    pub symlinks: bool,

    /// Optional path to an SSL/TLS certificate to serve with HTTPS
    #[arg(long = "ssl-cert", value_name = "path")]
    pub ssl_cert: Option<String>,

    /// Optional path to the SSL/TLS certificate's private key
    #[arg(long = "ssl-key", value_name = "path")]
    pub ssl_key: Option<String>,

    /// Optional path to the SSL/TLS certificate's passphrase
    #[arg(long = "ssl-pass", value_name = "path")]
    pub ssl_pass: Option<String>,

    /// Do not open a port other than the one specified when it's taken
    #[arg(long = "no-port-switching", action = clap::ArgAction::SetTrue)]
    pub no_port_switching: bool,

    /// Directory to serve
    #[arg(value_name = "directory", default_value = ".")]
    pub directory: String,
}

pub fn parse_args() -> CliArgs {
    CliArgs::parse()
}

pub fn print_help() {
    println!(r#"vaserve - Static file serving and directory listing

  USAGE

    $ vaserve --help
    $ vaserve --version
    $ vaserve folder_name
    $ vaserve [-l listen_uri [-l ...]] [directory]

    By default, vaserve will listen on 0.0.0.0:3000 and serve the
    current working directory on that address.

    Specifying a single --listen argument will overwrite the default, not supplement it.

  OPTIONS

    --help                              Shows this help message

    -v, --version                       Displays the current version of vaserve

    -l, --listen listen_uri             Specify a URI endpoint on which to listen (see below) -
                                        more than one may be specified to listen in multiple places

    -p                                  Specify custom port

    -s, --single                        Rewrite all not-found requests to `index.html`

    -d, --debug                         Show debugging information

    -c, --config                        Specify custom path to `serve.json`

    -L, --no-request-logging            Do not log any request information to the console.

    -C, --cors                          Enable CORS, sets `Access-Control-Allow-Origin` to `*`

    -n, --no-clipboard                  Do not copy the local address to the clipboard

    -u, --no-compression                Do not compress files

    --no-etag                           Send `Last-Modified` header instead of `ETag`

    -S, --symlinks                      Resolve symlinks instead of showing 404 errors

    --ssl-cert                          Optional path to an SSL/TLS certificate to serve with HTTPS
                                        Supported formats: PEM (default) and PKCS12 (PFX)

    --ssl-key                           Optional path to the SSL/TLS certificate's private key
                                        Applicable only for PEM certificates

    --ssl-pass                          Optional path to the SSL/TLS certificate's passphrase

    --no-port-switching                 Do not open a port other than the one specified when it's taken."#);
}
