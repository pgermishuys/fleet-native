//! `latest.json`, the file the release's publish step writes beside the three downloads (`scripts/publish-release.sh`):
//! the build number, when it was made, and for each platform the asset's name, size and SHA-256. Pure: no network.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asset {
    pub name: String,
    pub size: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub commit: String,
    /// "2026-11-15T14:19:00Z".
    pub built_at: String,
    pub build: u64,
    pub assets: BTreeMap<String, Asset>,
}

/// How this copy of the app was installed, which decides the package an update comes in and whether it can replace itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// A folder the app can rewrite: the Linux tar, the Windows zip or setup, the macOS bundle.
    Folder,
    /// Linux: running from an AppImage; an update replaces that one file (the server is inside it).
    AppImage(PathBuf),
    /// Linux: installed by the `.deb` into `/opt`, which only the package manager may write.
    Deb,
}

/// Where `scripts/make-deb.sh` puts the program.
pub const DEB_EXE: &str = "/opt/Fleet-Native/fleet-native";

pub fn kind_of(os: &str, exe: Option<&Path>, appimage: Option<&str>) -> Kind {
    if os == "linux" {
        if let Some(file) = appimage.filter(|a| !a.is_empty()) {
            return Kind::AppImage(PathBuf::from(file));
        }
        if exe == Some(Path::new(DEB_EXE)) {
            return Kind::Deb;
        }
    }
    Kind::Folder
}

/// How this process was installed (the AppImage runtime sets `APPIMAGE`).
pub fn install_kind() -> Kind {
    kind_of(std::env::consts::OS, std::env::current_exe().ok().as_deref(), std::env::var("APPIMAGE").ok().as_deref())
}

/// The key of this install's asset in `latest.json`; `None` where no build is published (a 32-bit or exotic target).
pub fn platform_key() -> Option<&'static str> {
    key_for(std::env::consts::OS, std::env::consts::ARCH, &install_kind())
}

/// The old keys (`linux-x64`, `windows-x64`, `macos-arm64`) stay what builds already installed read; the Linux packages that
/// replace a file or only link have their own.
pub fn key_for(os: &str, arch: &str, kind: &Kind) -> Option<&'static str> {
    match (os, arch, kind) {
        ("windows", "x86_64", _) => Some("windows-x64"),
        ("macos", "aarch64", _) => Some("macos-arm64"),
        ("linux", "x86_64", Kind::AppImage(_)) => Some("linux-x64-appimage"),
        ("linux", "x86_64", Kind::Deb) => Some("linux-x64-deb"),
        ("linux", "x86_64", Kind::Folder) => Some("linux-x64"),
        _ => None,
    }
}

pub fn parse(json: &str) -> Result<Manifest, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("latest.json is not JSON: {e}"))?;
    let build = v.get("build").and_then(Value::as_u64).ok_or("latest.json has no build number")?;
    let text = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let mut assets = BTreeMap::new();
    for (key, a) in v.get("assets").and_then(Value::as_object).ok_or("latest.json has no assets")? {
        let name = a.get("name").and_then(Value::as_str).unwrap_or("");
        let sha256 = a.get("sha256").and_then(Value::as_str).unwrap_or("");
        let size = a.get("size").and_then(Value::as_u64).unwrap_or(0);
        // The name becomes part of a URL and a file name: one plain file name, nothing that climbs out of a folder.
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') || sha256.len() != 64 || size == 0 {
            continue;
        }
        assets.insert(key.clone(), Asset { name: name.to_string(), size, sha256: sha256.to_ascii_lowercase() });
    }
    Ok(Manifest { commit: text("commit"), built_at: text("built_at"), build, assets })
}

/// The build number this binary was made as (`FLEET_NATIVE_BUILD`, baked in by CI), `None` for a local `dev` build.
///
/// A debug build can be told another number with `FLEET_NATIVE_TEST_BUILD` (scripts/update-check.sh: CI proves the replace step
/// on a build that believes it is old, without a second build of everything). A release build ignores it.
pub fn running_build() -> Option<u64> {
    #[cfg(debug_assertions)]
    if let Ok(v) = std::env::var("FLEET_NATIVE_TEST_BUILD") {
        return v.parse().ok();
    }
    option_env!("FLEET_NATIVE_BUILD").and_then(|b| b.parse().ok())
}

/// A newer build than the running one. A `dev` build never updates.
pub fn is_newer(running: Option<u64>, latest: u64) -> bool {
    running.is_some_and(|r| latest > r)
}

/// "15 November 2026" from "2026-11-15T14:19:00Z"; the text as it came when it isn't that shape.
pub fn date_words(built_at: &str) -> String {
    const MONTHS: [&str; 12] =
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
    let mut parts = built_at.get(..10).unwrap_or("").split('-');
    match (parts.next().and_then(|y| y.parse::<u32>().ok()), parts.next().and_then(|m| m.parse::<usize>().ok()), parts.next()) {
        (Some(y), Some(m @ 1..=12), Some(d)) => match d.parse::<u32>() {
            Ok(d) => format!("{d} {} {y}", MONTHS[m - 1]),
            Err(_) => built_at.to_string(),
        },
        _ => built_at.to_string(),
    }
}

/// Feeds `read` through SHA-256 and returns the lowercase hex and the byte count.
pub fn sha256_of(mut read: impl Read) -> std::io::Result<(String, u64)> {
    let mut hash = Sha256::new();
    let (mut buf, mut total) = (vec![0u8; 64 * 1024], 0u64);
    loop {
        let n = read.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        total += n as u64;
    }
    Ok((hash.finalize().iter().map(|b| format!("{b:02x}")).collect(), total))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"commit":"abc1234def","built_at":"2026-11-15T14:19:00Z","build":200,"assets":{
      "windows-x64":{"name":"fleet-native-windows-x64.zip","size":10,"sha256":"AA00000000000000000000000000000000000000000000000000000000000000"},
      "macos-arm64":{"name":"fleet-native-macos-arm64.dmg","size":20,"sha256":"bb00000000000000000000000000000000000000000000000000000000000000"},
      "linux-x64":{"name":"fleet-native-linux-x64.tar.gz","size":30,"sha256":"cc00000000000000000000000000000000000000000000000000000000000000"}}}"#;

    #[test]
    fn parses_the_published_shape() {
        let m = parse(SAMPLE).unwrap();
        assert_eq!((m.build, m.commit.as_str()), (200, "abc1234def"));
        assert_eq!(m.assets.len(), 3);
        let w = &m.assets["windows-x64"];
        assert_eq!((w.name.as_str(), w.size), ("fleet-native-windows-x64.zip", 10));
        assert!(w.sha256.starts_with("aa00"), "the checksum is compared in lowercase");
    }

    #[test]
    fn refuses_what_is_not_a_release() {
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"assets":{}}"#).is_err(), "no build number");
        assert!(parse(r#"{"build":3}"#).is_err(), "no assets");
    }

    #[test]
    fn drops_assets_that_could_not_be_trusted() {
        let bad = r#"{"build":3,"assets":{
          "linux-x64":{"name":"../evil.tar.gz","size":1,"sha256":"cc00000000000000000000000000000000000000000000000000000000000000"},
          "macos-arm64":{"name":"ok.dmg","size":1,"sha256":"short"},
          "windows-x64":{"name":"a\\b.zip","size":1,"sha256":"cc00000000000000000000000000000000000000000000000000000000000000"}}}"#;
        assert!(parse(bad).unwrap().assets.is_empty());
    }

    #[test]
    fn newer_means_a_higher_build_and_dev_never_updates() {
        assert!(is_newer(Some(199), 200));
        assert!(!is_newer(Some(200), 200));
        assert!(!is_newer(Some(201), 200));
        assert!(!is_newer(None, 200), "a dev build never checks or offers");
    }

    #[test]
    fn picks_the_asset_for_the_platform() {
        let f = Kind::Folder;
        assert_eq!(key_for("windows", "x86_64", &f), Some("windows-x64"));
        assert_eq!(key_for("macos", "aarch64", &f), Some("macos-arm64"));
        assert_eq!(key_for("linux", "x86_64", &f), Some("linux-x64"));
        assert_eq!(key_for("linux", "aarch64", &f), None);
        assert_eq!(key_for("macos", "x86_64", &f), None);
        let m = parse(SAMPLE).unwrap();
        assert_eq!(m.assets[key_for("linux", "x86_64", &f).unwrap()].name, "fleet-native-linux-x64.tar.gz");
    }

    #[test]
    fn each_package_kind_has_its_own_asset() {
        let image = kind_of("linux", Some(Path::new("/tmp/.mount_x/usr/bin/fleet-native")), Some("/home/u/Fleet-Native.AppImage"));
        assert_eq!(image, Kind::AppImage(PathBuf::from("/home/u/Fleet-Native.AppImage")));
        assert_eq!(key_for("linux", "x86_64", &image), Some("linux-x64-appimage"));
        let deb = kind_of("linux", Some(Path::new(DEB_EXE)), None);
        assert_eq!(deb, Kind::Deb);
        assert_eq!(key_for("linux", "x86_64", &deb), Some("linux-x64-deb"));
        assert_eq!(kind_of("linux", Some(Path::new("/home/u/fleet-native/fleet-native")), None), Kind::Folder, "an unpacked tar");
        assert_eq!(kind_of("linux", Some(Path::new(DEB_EXE)), Some("")), Kind::Deb, "an empty APPIMAGE is not an AppImage");
        assert_eq!(kind_of("windows", Some(Path::new(DEB_EXE)), Some("x")), Kind::Folder, "only Linux has these kinds");
    }

    #[test]
    fn the_date_in_words() {
        assert_eq!(date_words("2026-11-15T14:19:00Z"), "15 November 2026");
        assert_eq!(date_words("2026-01-02T00:00:00Z"), "2 January 2026");
        assert_eq!(date_words("soon"), "soon");
    }

    #[test]
    fn the_checksum_of_known_bytes() {
        let (hex, n) = sha256_of(&b"abc"[..]).unwrap();
        assert_eq!(hex, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(n, 3);
    }
}
