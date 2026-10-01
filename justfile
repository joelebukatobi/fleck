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
    install -Dm0644 data/icons/iconoir/page-edit.svg {{icon-dst}}
    install -Dm0644 data/icons/iconoir/page-edit.svg {{applet-icon-dst}}

uninstall:
    rm -f {{bin-dst}} {{desktop-dst}} {{applet-desktop-dst}} {{autostart-dst}} \
        {{metainfo-dst}} {{dbus-dst}} {{icon-dst}} {{applet-icon-dst}}

# Install for this user only: no root, and reminders start with the session.
install-user: build-release
    install -Dm0755 {{bin-src}} {{user-bin-dst}}
    install -Dm0644 data/{{appid}}.desktop {{user-share}}/applications/{{appid}}.desktop
    install -Dm0644 data/{{appletid}}.desktop {{user-share}}/applications/{{appletid}}.desktop
    install -Dm0644 data/{{appid}}.metainfo.xml {{user-share}}/metainfo/{{appid}}.metainfo.xml
    install -Dm0644 data/icons/iconoir/page-edit.svg {{user-share}}/icons/hicolor/scalable/apps/{{appid}}.svg
    install -Dm0644 data/icons/iconoir/page-edit.svg {{user-share}}/icons/hicolor/scalable/apps/{{appletid}}-symbolic.svg
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

# The speech model dictation needs, about 142 MB, downloaded once.
model:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -s "{{model}}" ]; then echo "the speech model is already here"; exit 0; fi
    mkdir -p "{{model-dir}}"
    curl -fL --progress-bar "{{model-url}}" -o "{{model}}"
    echo "speech model installed"
