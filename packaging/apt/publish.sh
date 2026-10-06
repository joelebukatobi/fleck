#!/usr/bin/env bash
# Builds the apt repository Pages serves: puts a .deb in the pool,
# regenerates the indexes, and signs them. Old versions stay where they are, so
# `apt install fleck=0.1.0` keeps working after a bad release.
#
#   publish.sh <repo-dir> <package.deb> [changelog]
#
# The signing key must already be in the keyring this runs with; nothing here
# touches a key, it only asks gpg to sign.
set -euo pipefail

repo="${1:?the directory holding the apt repository}"
deb="${2:?the .deb to publish}"
changelog="${3:-}"

suite="stable"
component="main"
arch="$(dpkg-deb --field "${deb}" Architecture)"
version="$(dpkg-deb --field "${deb}" Version)"
dist="${repo}/dists/${suite}"
binary="${dist}/${component}/binary-${arch}"

mkdir -p "${repo}/pool/${component}/f/fleck" "${binary}"
install -m0644 "${deb}" "${repo}/pool/${component}/f/fleck/"

# Paths inside Packages have to be relative to the repository root, so the
# indexes are built from there.
cd "${repo}"
apt-ftparchive --arch "${arch}" packages pool > "dists/${suite}/${component}/binary-${arch}/Packages"
gzip -9fkn "dists/${suite}/${component}/binary-${arch}/Packages"

# apt-ftparchive reads the suite's own fields from here rather than guessing.
cat > /tmp/fleck-ftparchive-release.conf <<CONF
APT::FTPArchive::Release::Origin "Fleck";
APT::FTPArchive::Release::Label "Fleck";
APT::FTPArchive::Release::Suite "${suite}";
APT::FTPArchive::Release::Codename "${suite}";
APT::FTPArchive::Release::Architectures "${arch}";
APT::FTPArchive::Release::Components "${component}";
APT::FTPArchive::Release::Description "Sticky notes for the COSMIC desktop";
CONF
apt-ftparchive -c /tmp/fleck-ftparchive-release.conf release "dists/${suite}" > "dists/${suite}/Release.tmp"
mv "dists/${suite}/Release.tmp" "dists/${suite}/Release"

# Both signatures: InRelease for apt, Release.gpg for anything older.
rm -f "dists/${suite}/InRelease" "dists/${suite}/Release.gpg"
gpg --batch --yes --clearsign --output "dists/${suite}/InRelease" "dists/${suite}/Release"
gpg --batch --yes --detach-sign --armor --output "dists/${suite}/Release.gpg" "dists/${suite}/Release"

# The changelog grows: this release's notes go on top of what is already there.
if [ -n "${changelog}" ]; then
    notes="$(cat "${changelog}")"
    {
        printf '# Fleck releases\n\n%s\n' "${notes}"
        if [ -f CHANGELOG.md ]; then
            tail -n +2 CHANGELOG.md | sed '1{/^$/d}'
        fi
    } > CHANGELOG.md.new
    mv CHANGELOG.md.new CHANGELOG.md
fi

echo "published fleck ${version} (${arch})"
