name := 'fleck'
appid := 'io.github.joelebukatobi.Fleck'
appletid := 'io.github.joelebukatobi.FleckApplet'

rootdir := ''
prefix := '/usr'
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
bin-src := cargo-target-dir / 'release' / name

# Where a system install puts everything.
bin-dst := base-dir / 'bin' / name
desktop-dst := base-dir / 'share' / 'applications' / appid + '.desktop'
applet-desktop-dst := base-dir / 'share' / 'applications' / appletid + '.desktop'
autostart-dst := base-dir / 'share' / 'applications' / appid + '-autostart.desktop'
metainfo-dst := base-dir / 'share' / 'metainfo' / appid + '.metainfo.xml'
dbus-dst := base-dir / 'share' / 'dbus-1' / 'services' / appid + '.service'
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'
applet-icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appletid + '-symbolic.svg'

# Where a single-user install puts everything.
user-bin-dst := env('HOME') / '.local' / 'bin' / name
user-share := env('XDG_DATA_HOME', env('HOME') / '.local' / 'share')
user-config := env('XDG_CONFIG_HOME', env('HOME') / '.config')
model-dir := user-share / name / 'models'
model := model-dir / 'ggml-base.bin'
model-url := 'https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin'

default: build-release

# Everything CI runs. Run this before committing.
check:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    just validate

build-debug *args:
    cargo build {{args}}

build-release *args: (build-debug '--release' args)

run *args: build-release
    {{bin-src}} {{args}}

clean:
    cargo clean

# Desktop entries and the AppStream file must be valid, or stores ignore them.
# COSMIC applets use the unregistered "COSMIC" category, which is expected.
validate:
    #!/usr/bin/env bash
    set -euo pipefail
    output="$(desktop-file-validate data/*.desktop || true)"
    output="$(printf '%s\n' "${output}" | grep -v 'unregistered value "COSMIC"' \
        | grep -v 'does not contain a registered main category' | grep -v '^$' || true)"
    if [ -n "${output}" ]; then printf '%s\n' "${output}"; exit 1; fi
    appstreamcli validate --no-net data/{{appid}}.metainfo.xml
    echo "metadata ok"

# Install for everyone. Needs root; `just rootdir=pkg install` stages instead.
install:
    install -Dm0755 {{bin-src}} {{bin-dst}}
    install -Dm0644 data/{{appid}}.desktop {{desktop-dst}}
    install -Dm0644 data/{{appletid}}.desktop {{applet-desktop-dst}}
    install -Dm0644 data/{{appid}}-autostart.desktop {{autostart-dst}}
    install -Dm0644 data/{{appid}}.metainfo.xml {{metainfo-dst}}
    install -Dm0644 data/{{appid}}.service {{dbus-dst}}
    install -Dm0644 data/icons/fleck/io.github.joelebukatobi.Fleck.svg {{icon-dst}}
    install -Dm0644 data/icons/fleck/io.github.joelebukatobi.Fleck-symbolic.svg {{applet-icon-dst}}

uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{applet-desktop-dst}} {{autostart-dst}} \
        {{metainfo-dst}} {{dbus-dst}} {{icon-dst}} {{applet-icon-dst}}

# Install for this user only: no root, and reminders start with the session.
install-user: build-release
    install -Dm0755 {{bin-src}} {{user-bin-dst}}
    install -Dm0644 data/{{appid}}.desktop {{user-share}}/applications/{{appid}}.desktop
    install -Dm0644 data/{{appletid}}.desktop {{user-share}}/applications/{{appletid}}.desktop
    install -Dm0644 data/{{appid}}.metainfo.xml {{user-share}}/metainfo/{{appid}}.metainfo.xml
    install -Dm0644 data/icons/fleck/io.github.joelebukatobi.Fleck.svg {{user-share}}/icons/hicolor/scalable/apps/{{appid}}.svg
    install -Dm0644 data/icons/fleck/io.github.joelebukatobi.Fleck-symbolic.svg {{user-share}}/icons/hicolor/scalable/apps/{{appletid}}-symbolic.svg
    install -Dm0644 data/{{appid}}-autostart.desktop {{user-config}}/autostart/{{appid}}-autostart.desktop
    sed "s|^Exec=.*|Exec={{user-bin-dst}}|" data/{{appid}}.service \
        > {{user-share}}/dbus-1/services/{{appid}}.service
    @echo "Installed. Add the panel icon in COSMIC Settings: Desktop, Panel, Configure panel applets."

uninstall-user:
    rm -f {{user-bin-dst}} \
        {{user-share}}/applications/{{appid}}.desktop \
        {{user-share}}/applications/{{appletid}}.desktop \
        {{user-share}}/metainfo/{{appid}}.metainfo.xml \
        {{user-share}}/icons/hicolor/scalable/apps/{{appid}}.svg \
        {{user-share}}/icons/hicolor/scalable/apps/{{appletid}}-symbolic.svg \
        {{user-share}}/dbus-1/services/{{appid}}.service \
        {{user-config}}/autostart/{{appid}}-autostart.desktop

# Build the apt repository locally, the way the release workflow builds the one
# `gh-pages` serves, and let apt read it back. Signs with whatever key gpg picks
# by default, so it needs a signing key in your keyring.
apt-repo: deb
    #!/usr/bin/env bash
    set -euo pipefail
    repo="{{cargo-target-dir}}/apt"
    mkdir -p "${repo}"
    packaging/apt/changelog.sh > "{{cargo-target-dir}}/notes.md"
    packaging/apt/publish.sh "${repo}" {{cargo-target-dir}}/deb/*.deb "{{cargo-target-dir}}/notes.md"
    cp packaging/apt/index.html "${repo}/index.html"
    echo "apt repository at ${repo}"

# The speech model dictation needs, about 142 MB, downloaded once.
model:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -s "{{model}}" ]; then echo "the speech model is already here"; exit 0; fi
    mkdir -p "{{model-dir}}"
    curl -fL --progress-bar "{{model-url}}" -o "{{model}}"
    echo "speech model installed"

# A .deb for Pop!_OS, Ubuntu and Debian, built from the same files `install`
# uses. dpkg-deb comes with the system, so there is no packaging toolchain to
# install; CI attaches the result to the release.
deb: build-release
    #!/usr/bin/env bash
    set -euo pipefail
    version="$(cargo metadata --no-deps --format-version 1 \
        | grep -o '"name":"fleck","version":"[^"]*"' | head -1 | cut -d'"' -f8)"
    arch="$(dpkg --print-architecture)"
    stage="{{cargo-target-dir}}/deb/{{name}}_${version}_${arch}"
    rm -rf "${stage}"
    just rootdir="${stage}" prefix=/usr install
    # Debian expects stripped binaries; the debug symbols are most of the size.
    strip --strip-unneeded "${stage}/usr/bin/{{name}}"
    # A system install puts the autostart entry where nothing reads it; in a
    # package it belongs in xdg's own directory, so reminders fire after login
    # without anyone having to copy a file.
    rm -f "${stage}/usr/share/applications/{{appid}}-autostart.desktop"
    install -Dm0644 data/{{appid}}-autostart.desktop \
        "${stage}/etc/xdg/autostart/{{appid}}-autostart.desktop"
    mkdir -p "${stage}/DEBIAN"
    # Depends covers what `ldd` finds; Wayland, Vulkan and the fonts are opened
    # at runtime by libcosmic and come with every COSMIC desktop. The recorders
    # are only needed for dictation, and the speech model is a separate download.
    cat > "${stage}/DEBIAN/control" <<CONTROL
    Package: {{name}}
    Version: ${version}
    Architecture: ${arch}
    Maintainer: Joel Onwuanaku <joelebuka@gmail.com>
    Section: x11
    Priority: optional
    Depends: libc6, libgcc-s1, libstdc++6, libxkbcommon0
    Recommends: pipewire-bin | pulseaudio-utils
    Homepage: https://github.com/joelebukatobi/fleck
    Description: Sticky notes for the COSMIC desktop
     Notes that stay where you put them, with reminders, images and
     dictation. Comes with a panel applet for opening them.
    CONTROL
    sed -i 's/^    //' "${stage}/DEBIAN/control"
    dpkg-deb --root-owner-group --build "${stage}" > /dev/null
    echo "{{cargo-target-dir}}/deb/{{name}}_${version}_${arch}.deb"

# The Flatpak build, for the COSMIC Store. Needs flatpak-builder and, the first
# time, a few GB of runtimes:
#   sudo apt install flatpak-builder
#   flatpak install --user flathub org.freedesktop.Platform//25.08 \
#       org.freedesktop.Sdk//25.08 org.freedesktop.Sdk.Extension.rust-stable//25.08 \
#       org.freedesktop.Sdk.Extension.llvm21//25.08
#   flatpak install --user cosmic com.system76.Cosmic.BaseApp
flatpak-id := 'io.github.joelebukatobi.Fleck'
flatpak-manifest := 'flatpak' / flatpak-id + '.json'

# Build and install the Flatpak from the working tree, rather than from a tag,
# so a change can be tried before it is released.
flatpak:
    #!/usr/bin/env bash
    set -euo pipefail
    build="{{cargo-target-dir}}/flatpak"
    mkdir -p "${build}"
    manifest="${build}/{{flatpak-id}}.json"
    # The released manifest builds from a tag; this one builds what is here.
    # `skip` matters: a dir source copies everything, and target/ is tens of GB.
    python3 -c 'import json,sys; m=json.load(open(sys.argv[1])); m["modules"][-1]["sources"][0]={"type":"dir","path":sys.argv[3],"skip":["target",".flatpak-builder",".git",".venv"]}; json.dump(m,open(sys.argv[2],"w"),indent=2)' \
        "{{flatpak-manifest}}" "${manifest}" "${PWD}"
    cp flatpak/cargo-sources.json "${build}/cargo-sources.json"
    flatpak-builder --force-clean --user --install "${build}/state" "${manifest}"
    echo "Installed. Run it with: flatpak run {{flatpak-id}}"

# Regenerate the crate list the Flatpak build fetches offline. Run it whenever
# Cargo.lock changes. Needs aiohttp, toml and tomlkit:
#   python3 -m venv .venv && .venv/bin/pip install aiohttp toml tomlkit
#   curl -fsSLO https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py
flatpak-sources generator='flatpak-cargo-generator.py' python='.venv/bin/python':
    {{python}} {{generator}} Cargo.lock -o flatpak/cargo-sources.json
