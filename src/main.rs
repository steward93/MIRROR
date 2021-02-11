/*
 * Copyright (c) 2017 Pascal Bach
 *
 * SPDX-License-Identifier:     MIT
 */

use std::cmp;
use std::env;

// Used for error and debug logging
use env_logger::Env;
use log::{debug, error, info};

// Used to do command line parsing
use std::path::PathBuf;
use structopt::clap::{arg_enum, crate_name, crate_version};
use structopt::StructOpt;

// Load the real functionality
use git_mirror::do_mirror;
use git_mirror::provider::{GitHub, GitLab, Provider};
use git_mirror::MirrorOptions;

use std::process::exit;

arg_enum! {
    #[derive(Debug)]
    enum Providers {
      GitLab,
      GitHub
    }
}

/// command line options
#[derive(StructOpt, Debug)]
#[structopt(name = "git-mirror")]
struct Opt {
    /// Provider to use for fetching repositories
    #[structopt(
        long = "provider",
        short = "p",
        default_value = "GitLab",
        possible_values = &Providers::variants(),
        case_insensitive = true
    )]
    provider: Providers,

    /// URL of the instance to get repositories from
    #[structopt(
        long = "url",
        short = "u",
        default_value_ifs(&[
            ("provider", Some("GitLab"), "https://gitlab.com"),
            ("provider", Some("GitHub"), "https://api.github.com"),
        ])
    )]
    url: String,

    /// Name of the group to check for repositories to sync
    #[structopt(long = "group", short = "g")]
    group: String,

    /// Directory where the local clones are stored
    #[structopt(long = "mirror-dir", short = "m", default_value = "./mirror-dir")]
    mirror_dir: PathBuf,

    /// Verbosity level
    #[structopt(short, long, parse(from_occurrences))]
    verbose: u8,

    /// Use http(s) instead of SSH to sync the GitLab repository
    #[structopt(long)]
    http: bool,

    /// Only print what to do without actually running any git commands
    #[structopt(long)]
    dry_run: bool,

    /// Number of concurrent mirror jobs
    #[structopt(short = "c", long, default_value = "1")]
    worker_count: usize,

    /// Location where to store metrics for consumption by
    /// Prometheus node exporter's text file colloctor
    #[structopt(long)]
    metric_file: Option<PathBuf>,

    /// Location where to store the Junit XML report
    #[structopt(long)]
    junit_report: Option<PathBuf>,

    /// Git executable to use
    #[structopt(long, default_value = "git")]
    git_executable: String,

    /// Private token or Personal access token to access the GitLab or GitHub API
    #[structopt(long, env = "PRIVATE_TOKEN")]
    private_token: Option<String>,

    /// Default refspec used to mirror repositories, can be overridden per project
    #[structopt(long)]
    refspec: Option<Vec<String>>,

    /// Remove the local working repository after pushing. This requires a full re-clone on the next run.
    #[structopt(long)]
    remove_workrepo: bool,
}

impl Into<MirrorOptions> for Opt {
    fn into(self) -> MirrorOptions {
        MirrorOptions {
            mirror_dir: self.mirror_dir,
            dry_run: self.dry_run,
            worker_count: self.worker_count,
            metrics_file: self.metric_file,
            junit_file: self.junit_report,
            git_executable: self.git_executable,
            refspec: self.refspec,
            remove_workrepo: self.remove_workrepo,
        }
    }
}

fn main() {
    // Setup commandline parser
    let opt = Opt::from_args();
    debug!("{:#?}", opt);

    let env_log_level = match cmp::min(opt.verbose, 4) {
        4 => "git_mirror=trace",
        3 => "git_mirror=debug",
        2 => "git_mirror=info",
        1 => "git_mirror=warn",
        _ => "git_mirror=error",
    };
    env_logger::Builder::from_env(Env::default().default_filter_or(env_log_level)).init();

    // Run OpenSSL probing on all platforms even the ones not using it
    openssl_probe::init_ssl_cert_env_vars();

    let provider: Box<dyn Provider> = match opt.provider {
        Providers::GitLab => Box::new(GitLab {
            url: opt.url.to_owned(),
            group: opt.group.to_owned(),
            use_http: opt.http,
            private_token: opt.private_token.to_owned(),
            recursive: true,
        }),
        Providers::GitHub => Box::new(GitHub {
            url: opt.url.to_owned(),
            org: opt.group.to_owned(),
            use_http: opt.http,
            private_token: opt.private_token.to_owned(),
            useragent: format!("{}/{}", crate_name!(), crate_version!()),
        }),
    };

    let opts: MirrorOptions = opt.into();

    match do_mirror(provider, &opts) {
        Ok(_) => {
            info!("All done");
        }
        Err(e) => {
            error!("Error occured: {}", e);
            exit(2); // TODO: Return code in erro
        }
    };
}
