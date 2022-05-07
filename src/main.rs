use std::{env, io, path::PathBuf};

use clap::Parser;
use fuser::MountOption;

use drive::{model::Credentials,PikpakDrive, DriveConfig};
use vfs::PikpakDriveFileSystem;




mod drive;
mod error;
mod file_cache;
mod vfs;

#[derive(Parser, Debug)]
#[clap(name = "PikpakDrive-fuse", about, version, author)]
struct Opt {
    /// Mount point
    #[clap(parse(from_os_str))]
    path: PathBuf,
    /// Pikpak drive refresh token
    // #[clap(short, long, env = "REFRESH_TOKEN", default_value = "")]
    // refresh_token: String,


    #[structopt(long, env = "PIKPAK_USER")]
    pikpak_user: String,

    #[structopt(long, env = "PIKPAK_PASSWORD")]
    pikpak_password: String,

    #[structopt(long, env = "PROXY_URL", default_value = "")]
    proxy_url: String,


    /// Working directory, refresh_token will be stored in there if specified
    #[clap(short = 'w', long)]
    workdir: Option<PathBuf>,
    /// Pikpak PDS domain id
    #[clap(long)]
    domain_id: Option<String>,
    /// Allow other users to access the drive
    #[clap(long)]
    allow_other: bool,
    /// Read/download buffer size in bytes, defaults to 10MB
    #[clap(short = 'S', long, default_value = "10485760")]
    read_buffer_size: usize,
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "native-tls-vendored")]
    openssl_probe::init_ssl_cert_env_vars();

    if env::var("RUST_LOG").is_err() {
        env::set_var("RUST_LOG", "PikpakDrive_fuse=info");
    }
    tracing_subscriber::fmt::init();

    let opt = Opt::parse();
    // let drive_config = if let Some(domain_id) = opt.domain_id {
    //     DriveConfig {
    //         api_base_url: format!("https://{}.api.aliyunpds.com", domain_id),
    //         refresh_token_url: format!("https://{}.auth.aliyunpds.com/v2/account/token", domain_id),
    //         workdir: opt.workdir,
    //     }
    // } else {
    //     DriveConfig {
    //         api_base_url: "https://api.PikpakDrive.com".to_string(),
    //         refresh_token_url: "https://websv.PikpakDrive.com/token/refresh".to_string(),
    //         workdir: opt.workdir,
    //     }
    // };

    let drive_config = DriveConfig {
        api_base_url: "https://api-drive.mypikpak.com/drive/v1/files".to_string(),
        refresh_token_url: "https://user.mypikpak.com/v1/auth/signin".to_string(),
        workdir: opt.workdir,
    };

    let credentials = Credentials{
        username:opt.pikpak_user,
        password:opt.pikpak_password,
    };


    let drive = PikpakDrive::new(drive_config,credentials).map_err(|_| {
        io::Error::new(io::ErrorKind::Other, "initialize PikpakDrive client failed")
    })?;

    let _nick_name = drive.nick_name.clone();
    let vfs = PikpakDriveFileSystem::new(drive, opt.read_buffer_size);
    let mut mount_options = vec![MountOption::AutoUnmount, MountOption::NoAtime];
    // read only for now
    mount_options.push(MountOption::RO);
    if opt.allow_other {
        mount_options.push(MountOption::AllowOther);
    }
    if cfg!(target_os = "macos") {
        mount_options.push(MountOption::CUSTOM("local".to_string()));
        mount_options.push(MountOption::CUSTOM("noappledouble".to_string()));
        let volname = if let Some(nick_name) = _nick_name {
            format!("volname=PikPak网盘({})", nick_name)
        } else {
            "volname=PikPak网盘".to_string()
        };
        mount_options.push(MountOption::CUSTOM(volname));
    }
    fuser::mount2(vfs, opt.path, &mount_options)?;
    Ok(())
}
