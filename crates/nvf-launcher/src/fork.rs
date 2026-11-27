//! Nuclear Sunset fork (FadedLight/NVForever-NuclearSunset): where this launcher looks for
//! updates and release news, and the build it says it is.
//!
//! Upstream's launcher checks upstream's releases and, on the Update button, swaps the
//! program files of the folder it runs from for upstream's archive (`update.rs`). Run from a
//! release folder that held this fork's build, that put upstream's launcher (no Mod
//! Organizer 2 card) in its place [observed 2026-11-25: the owner's release folder,
//! `update\installed.json` says 0.8.1, its `nvforever-launcher.exe` carries no
//! `Mo2Instance`; the `[Launcher]` keys in `NVForever.ini` were kept, only unread]. So this
//! fork's launcher reads this fork's releases only: an update can only bring this fork's
//! next build, never upstream's.
//!
//! The fork's releases are tagged `ns-<year>.<month>.<day>` (a second build of one day:
//! `.2` after the day) and carry `NVForever-<tag>-<target>.zip` plus `SHA256SUMS.txt`, the
//! shape `update.rs` installs. [`BUILD`] is raised with every release.

/// This fork on GitHub (the launcher's GITHUB button).
pub const REPO: &str = "https://github.com/FadedLight/NVForever-NuclearSunset";
/// This fork's release list (news and the update check).
pub const RELEASES: &str = "https://api.github.com/repos/FadedLight/NVForever-NuclearSunset/releases?per_page=10";
/// Where this fork's releases keep their files.
pub const DOWNLOADS: &str = "https://github.com/FadedLight/NVForever-NuclearSunset/releases/download/";
/// The fork's release tags start with this.
pub const TAG_PREFIX: &str = "ns-";
/// This build, as its release tag names it without [`TAG_PREFIX`].
pub const BUILD: &str = "2026.11.27";

/// The version a release tag names: `ns-2026.11.27` -> `2026.11.27` (`v…` as upstream's).
pub fn version_of_tag(tag: &str) -> &str {
    tag.strip_prefix(TAG_PREFIX).unwrap_or_else(|| tag.trim_start_matches('v'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_tags_compare_as_dates() {
        assert_eq!(version_of_tag("ns-2026.11.27"), "2026.11.27");
        assert_eq!(version_of_tag("v0.8.1"), "0.8.1");
        assert!(crate::news::is_newer(version_of_tag("ns-2026.11.27.2"), "2026.11.27"));
        assert!(crate::news::is_newer(version_of_tag("ns-2026.12.1"), BUILD));
        // The first two fork releases used a letter; they read as older than any later one.
        assert!(!crate::news::is_newer(version_of_tag("ns-2026.11.25b"), BUILD));
    }

    #[test]
    fn every_address_is_the_forks() {
        #[cfg(feature = "desktop")]
        assert_eq!(crate::update::REPO_DOWNLOADS, DOWNLOADS);
        for url in [REPO, RELEASES, DOWNLOADS, crate::news::RELEASES] {
            assert!(url.contains("FadedLight/NVForever-NuclearSunset"), "{url}");
        }
    }
}
