# 基本完成
命令行使用【可选参数--proxy-url xxxxxxxx】:
```
./pikpak-fuse --pikpak-user XXXXXXXXX --pikpak-password XXXXXXX  -w token保存目录  挂载点
```

# 挂载到本地Docker版不太实用就不说了

# 安装[可能需要给执行权限]
* 在[relase](https://github.com/ykxVK8yL5L/pikpak-fuse/releases)下载所需二进制，用命令行启动
* pip install pikpak-fuse


# pikpak-fuse

> 🚀 Help me to become a full-time open-source developer by [sponsoring me on GitHub](https://github.com/sponsors/ykxVK8yL5L)

pikpak网盘 FUSE 磁盘挂载，主要用于配合 [Emby](https://emby.media) 或者 [Jellyfin](https://jellyfin.org) 观看pikpak网盘内容，功能特性：

1. 目前只读，不支持写入
2. 支持 Linux 和 macOS，暂不支持 Windows

[pikpak-webdav](https://github.com/ykxVK8yL5L/pikpak-webdav) 项目已经实现了通过 WebDAV 访问pikpak网盘内容，但由于 Emby 和 Jellyfin 都不支持直接访问 WebDAV 资源，
需要配合 [rclone](https://rclone.org) 之类的软件将 WebDAV 挂载为本地磁盘，而本项目则直接通过 FUSE 实现将pikpak网盘挂载为本地磁盘，省去使用 rclone 再做一层中转。

## 安装

* macOS 需要先安装 [macfuse](https://osxfuse.github.io/)
* Linux 需要先安装 fuse
  * Debian 系如 Ubuntu: `apt-get install -y fuse3`
  * RedHat 系如 CentOS: `yum install -y fuse3`

可以从 [GitHub Releases](https://github.com/ykxVK8yL5L/pikpak-fuse/releases) 页面下载预先构建的二进制包， 也可以使用 pip 从 PyPI 下载:

```bash
pip install pikpak-fuse
```

如果系统支持 [Snapcraft](https://snapcraft.io) 比如 Ubuntu、Debian 等，也可以使用 snap 安装【未实现】：

```bash
sudo snap install pikpak-fuse
```

### OpenWrt 路由器

[GitHub Releases](https://github.com/ykxVK8yL5L/pikpak-fuse/releases) 中有预编译的 ipk 文件， 目前提供了
aarch64/arm/x86_64/i686 等架构的版本，可以下载后使用 opkg 安装，以 nanopi r4s 为例：

```bash
wget https://github.com/ykxVK8yL5L/pikpak-fuse/releases/download/v0.1.13/pikpak-fuse_0.1.13-1_aarch64_generic.ipk
wget https://github.com/ykxVK8yL5L/pikpak-fuse/releases/download/v0.1.13/luci-app-pikpak-fuse_0.1.13_all.ipk
wget https://github.com/ykxVK8yL5L/pikpak-fuse/releases/download/v0.1.13/luci-i18n-pikpak-fuse-zh-cn_0.1.13-1_all.ipk
opkg install pikpak-fuse_0.1.13-1_aarch64_generic.ipk
opkg install luci-app-pikpak-fuse_0.1.13_all.ipk
opkg install luci-i18n-pikpak-fuse-zh-cn_0.1.13-1_all.ipk
```

其它 CPU 架构的路由器可在 [GitHub Releases](https://github.com/ykxVK8yL5L/pikpak-fuse/releases) 页面中查找对应的架构的主程序 ipk 文件下载安装。

> Tips: 不清楚 CPU 架构类型可通过运行 `opkg print-architecture` 命令查询。

## 命令行用法

```bash
USAGE:
    pikpak-fuse [OPTIONS] --refresh-token <REFRESH_TOKEN> <PATH>

ARGS:
    <PATH>    Mount point

OPTIONS:
        --allow-other                            Allow other users to access the drive
        --domain-id <DOMAIN_ID>                  Aliyun PDS domain id
    -h, --help                                   Print help information
    --pikpak-user <Pikpak_USER>                  [env: Pikpak_USER=]
    --pikpak-password <Pikpak_PASSWORD>          [env: Pikpak_PASSWORD=]
    --proxy-url <PROXY_URL>                      [env: PROXY_URL=]
    
    -S, --read-buffer-size <READ_BUFFER_SIZE>    Read/download buffer size in bytes, defaults to 10MB [default: 10485760]
    -V, --version                                Print version information
    -w, --workdir <WORKDIR>                      Working directory, refresh_token will be stored in there if specified
```

比如将磁盘挂载到 `/mnt/pikpakDrive` 目录：

```bash
mkdir -p /mnt/pikpakDrive /var/run/pikpak-fuse
pikpak-fuse --pikpak-user XXXXXXXXX --pikpak-password XXXXXXX -w /var/run/pikpak-fuse /mnt/pikpakDrive
```

## Emby/Jellyfin

如果是直接运行在系统上的 Emby/Jellyfin，则可以直接在其控制台添加媒体库的时候选择pikpak网盘对应的挂载路径中的文件夹即可；
如果是 Docker 运行的 Emby/Jellyfin，则需要将pikpak网盘挂载路径也挂载到 Docker 容器中，假设pikpak网盘挂载路径为 `/mnt/pikpakDrive`，
以 Jellyfin 为例（假设 Jellyfin 工作路径为 `/root/jellyfin`）将云盘挂载到容器 `/media` 路径：

```bash
docker run -d --name jellyfin \
  -v /root/jellyfin/config:/config \
  -v /root/jellyfin/cache:/cache \
  -v /mnt/pikpakDrive:/media \
  -p 8096:8096 \
  --device=/dev/dri/renderD128 \
  --device /dev/dri/card0:/dev/dri/card0 \
  --restart unless-stopped \
  jellyfin/jellyfin
```

## License

This work is released under the MIT license. A copy of the license is provided in the [LICENSE](./LICENSE) file.
