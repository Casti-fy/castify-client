#!/usr/bin/env bash
# Build yt-dlp (YouTube-only) and minimal ffmpeg for Tauri sidecar bundling.
#
# yt-dlp is built from source with all extractors except YouTube removed,
# reducing the binary from ~33 MB to ~5-8 MB.
#
# Tauri sidecars require target-triple suffixed binaries:
#   yt-dlp-aarch64-apple-darwin
#   yt-dlp-x86_64-apple-darwin
#   ffmpeg-aarch64-apple-darwin
#   etc.
#
# Usage:
#   ./scripts/build-dependencies.sh                # Current platform
#   ./scripts/build-dependencies.sh --force         # Force rebuild
#   ./scripts/build-dependencies.sh --target <triple> # Specific target
#   ./scripts/build-dependencies.sh --yt-dlp-only   # Skip ffmpeg
#   ./scripts/build-dependencies.sh --ffmpeg-only    # Skip yt-dlp
#
# Requirements:
#   - Python 3.10+ with pip/venv
#   - For ffmpeg: C compiler, make

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARIES_DIR="$PROJECT_DIR/src-tauri/binaries"

# Versions
YTDLP_VERSION="2025.03.31"
FFMPEG_VERSION="7.1"
MACOS_MIN="12.0"

# Extractors to keep (add more here to support additional sites)
# YTDLP_KEEP_EXTRACTORS="youtube,spotify,soundcloud"
YTDLP_KEEP_EXTRACTORS="youtube"

# Parse arguments
FORCE=false
TARGET_TRIPLE=""
BUILD_FFMPEG=true
BUILD_YTDLP=true

for arg in "$@"; do
    case "$arg" in
        --force) FORCE=true ;;
        --ffmpeg-only) BUILD_YTDLP=false ;;
        --yt-dlp-only) BUILD_FFMPEG=false ;;
        --target)
            # Next arg will be the target
            ;;
        *)
            if [ -z "$TARGET_TRIPLE" ] && [[ "$arg" == *-* ]]; then
                TARGET_TRIPLE="$arg"
            fi
            ;;
    esac
done

# Auto-detect target triple if not specified
if [ -z "$TARGET_TRIPLE" ]; then
    TARGET_TRIPLE="$(rustc --print host-tuple 2>/dev/null || rustc -vV | grep host | awk '{print $2}')"
fi

echo "==> Target: $TARGET_TRIPLE"
echo "==> Binaries dir: $BINARIES_DIR"

mkdir -p "$BINARIES_DIR"

# Determine OS and arch from target triple
OS=""
ARCH=""
case "$TARGET_TRIPLE" in
    *apple-darwin*)
        OS="macos"
        case "$TARGET_TRIPLE" in
            aarch64-*) ARCH="arm64" ;;
            x86_64-*)  ARCH="x86_64" ;;
        esac
        ;;
    *linux-gnu*)
        OS="linux"
        case "$TARGET_TRIPLE" in
            aarch64-*) ARCH="aarch64" ;;
            x86_64-*)  ARCH="x86_64" ;;
        esac
        ;;
    *windows*)
        OS="windows"
        ARCH="x86_64"
        ;;
    *)
        echo "ERROR: Unsupported target triple: $TARGET_TRIPLE"
        exit 1
        ;;
esac

echo "==> OS=$OS ARCH=$ARCH"

# Detect Python command (Windows uses 'python', Unix uses 'python3')
if command -v python3 &>/dev/null; then
    PYTHON="python3"
elif command -v python &>/dev/null; then
    PYTHON="python"
else
    echo "ERROR: Python not found. Install Python 3.10+."
    exit 1
fi
echo "==> Python: $PYTHON"

# =============================================================================
# Build yt-dlp from source (YouTube-only)
# =============================================================================
if [ "$BUILD_YTDLP" = true ]; then
    YTDLP_OUT="$BINARIES_DIR/yt-dlp-${TARGET_TRIPLE}"
    [ "$OS" = "windows" ] && YTDLP_OUT="${YTDLP_OUT}.exe"

    if [ "$FORCE" = false ] && [ -f "$YTDLP_OUT" ]; then
        echo "yt-dlp already exists at $YTDLP_OUT. Use --force to rebuild."
    else
        echo ""
        echo "==> Building yt-dlp ${YTDLP_VERSION} (extractors: ${YTDLP_KEEP_EXTRACTORS}) for ${OS}/${ARCH}..."

        BUILD_DIR="$(mktemp -d)"
        # Clean up build dir on exit (but don't clobber ffmpeg's trap)
        YTDLP_BUILD_DIR="$BUILD_DIR"

        echo "==> Cloning yt-dlp ${YTDLP_VERSION}..."
        git clone --depth 1 --branch "${YTDLP_VERSION}" \
            https://github.com/yt-dlp/yt-dlp.git "$BUILD_DIR/yt-dlp"

        YTDLP_SRC="$BUILD_DIR/yt-dlp"

        echo "==> Setting up Python venv..."
        $PYTHON -m venv "$BUILD_DIR/venv"
        if [ "$OS" = "windows" ]; then
            source "$BUILD_DIR/venv/Scripts/activate"
        else
            source "$BUILD_DIR/venv/bin/activate"
        fi

        echo "==> Installing yt-dlp build dependencies..."
        $PYTHON "$YTDLP_SRC/devscripts/install_deps.py" -i pyinstaller

        echo "==> Pruning extractors (keeping: ${YTDLP_KEEP_EXTRACTORS})..."
        $PYTHON "$SCRIPT_DIR/prune-ytdlp-extractors.py" "$YTDLP_SRC" \
            --keep "$YTDLP_KEEP_EXTRACTORS"

        echo "==> Generating lazy extractors..."
        (cd "$YTDLP_SRC" && $PYTHON devscripts/make_lazy_extractors.py)

        echo "==> Building with PyInstaller..."
        (cd "$YTDLP_SRC" && $PYTHON -m bundle.pyinstaller)

        # Find the built binary (PyInstaller names vary by platform/arch)
        YTDLP_BIN="$(find "$YTDLP_SRC/dist" -maxdepth 1 -type f -name 'yt-dlp*' | head -1)"
        if [ -z "$YTDLP_BIN" ]; then
            echo "ERROR: No yt-dlp binary found in $YTDLP_SRC/dist/"
            ls -la "$YTDLP_SRC/dist/" 2>/dev/null || echo "(dist/ not found)"
            exit 1
        fi
        echo "    Built: $(basename "$YTDLP_BIN")"

        cp "$YTDLP_BIN" "$YTDLP_OUT"
        chmod +x "$YTDLP_OUT"

        deactivate 2>/dev/null || true
        rm -rf "$YTDLP_BUILD_DIR"

        echo "    yt-dlp: $(du -h "$YTDLP_OUT" | cut -f1) -> $(basename "$YTDLP_OUT")"
    fi
fi

# =============================================================================
# Build minimal ffmpeg from source
# =============================================================================
if [ "$BUILD_FFMPEG" = true ]; then
    FFMPEG_OUT="$BINARIES_DIR/ffmpeg-${TARGET_TRIPLE}"
    [ "$OS" = "windows" ] && FFMPEG_OUT="${FFMPEG_OUT}.exe"

    if [ "$FORCE" = false ] && [ -f "$FFMPEG_OUT" ]; then
        echo "ffmpeg already exists at $FFMPEG_OUT. Use --force to rebuild."
    else
        echo ""
        echo "==> Building minimal ffmpeg ${FFMPEG_VERSION} from source for ${OS}/${ARCH}..."

        BUILD_DIR="$(mktemp -d)"
        trap 'rm -rf "$BUILD_DIR"' EXIT

        FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-${FFMPEG_VERSION}.tar.xz"
        curl -L --fail --progress-bar -o "$BUILD_DIR/ffmpeg.tar.xz" "$FFMPEG_URL"

        echo "==> Extracting source..."
        tar xf "$BUILD_DIR/ffmpeg.tar.xz" -C "$BUILD_DIR"
        SRC_DIR="$BUILD_DIR/ffmpeg-${FFMPEG_VERSION}"

        # Minimal config: YouTube audio extraction -> M4A (AAC)
        # Only enable codecs needed for yt-dlp audio post-processing
        COMMON_FLAGS=(
            --disable-everything
            --disable-doc
            --disable-network
            --disable-autodetect
            --disable-x86asm

            --enable-ffmpeg
            --enable-small

            # Decoders: YouTube audio formats
            --enable-decoder=mp3,flac,pcm_s16le,pcm_s24le,pcm_s32le,vorbis,opus,aac,wmav2,mjpeg,png

            # Encoders: aac for M4A output
            --enable-encoder=aac,pcm_s16le

            # Demuxers
            --enable-demuxer=mp3,flac,wav,ogg,mov,aac,asf,concat,ffmetadata,image2,matroska

            # Muxers
            --enable-muxer=mov,wav

            # Parsers
            --enable-parser=aac,mpegaudio,flac,vorbis,opus,png,mjpeg

            # Protocols
            --enable-protocol=file,pipe

            # Filters
            --enable-filter=aresample,anull,aformat

            # BSF
            --enable-bsf=aac_adtstoasc
        )

        INSTALL_DIR="$BUILD_DIR/install"
        mkdir -p "$INSTALL_DIR"

        # Determine parallel job count
        if command -v nproc &>/dev/null; then
            JOBS="$(nproc)"
        elif command -v sysctl &>/dev/null; then
            JOBS="$(sysctl -n hw.ncpu)"
        else
            JOBS=4
        fi

        (
            cd "$SRC_DIR"

            case "$OS" in
                macos)
                    ./configure \
                        "${COMMON_FLAGS[@]}" \
                        --arch="$ARCH" \
                        --cc="clang -arch $ARCH" \
                        --enable-cross-compile \
                        --target-os=darwin \
                        --extra-cflags="-mmacosx-version-min=${MACOS_MIN}" \
                        --extra-ldflags="-mmacosx-version-min=${MACOS_MIN}" \
                        --prefix="$INSTALL_DIR"
                    ;;

                linux)
                    ./configure \
                        "${COMMON_FLAGS[@]}" \
                        --arch="$ARCH" \
                        --cc=gcc \
                        --target-os=linux \
                        --extra-ldflags="-static" \
                        --prefix="$INSTALL_DIR"
                    ;;

                windows)
                    # Native build inside MSYS2/MINGW on Windows CI
                    # For cross-compile from Linux/macOS, add --cross-prefix=x86_64-w64-mingw32-
                    UNAME_S="$(uname -s)"
                    CONFIGURE_EXTRA=()
                    case "$UNAME_S" in
                        MINGW*|MSYS*) ;; # native, no cross-prefix needed
                        *) CONFIGURE_EXTRA+=(--enable-cross-compile --cross-prefix=x86_64-w64-mingw32-) ;;
                    esac
                    ./configure \
                        "${COMMON_FLAGS[@]}" \
                        --arch=x86_64 \
                        --target-os=mingw64 \
                        "${CONFIGURE_EXTRA[@]}" \
                        --prefix="$INSTALL_DIR"
                    ;;
            esac

            make -j"$JOBS"
            make install
        )

        FFMPEG_BIN="$INSTALL_DIR/bin/ffmpeg"
        [ "$OS" = "windows" ] && FFMPEG_BIN="${FFMPEG_BIN}.exe"

        cp "$FFMPEG_BIN" "$FFMPEG_OUT"
        chmod +x "$FFMPEG_OUT"

        echo "    ffmpeg: $(du -h "$FFMPEG_OUT" | cut -f1) -> $(basename "$FFMPEG_OUT")"
    fi
fi

# =============================================================================
# Verify
# =============================================================================
echo ""
echo "==> Binaries in $BINARIES_DIR:"
ls -lh "$BINARIES_DIR"/ | grep -v '.gitkeep'

echo ""
echo "Done!"
