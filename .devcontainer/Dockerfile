# syntax=docker/dockerfile:1
# Build context is the repository root (see devcontainer.json), so paths below
# are relative to it. Only files allowed by /.dockerignore are visible.

ARG UV_VERSION=0.9
FROM ghcr.io/astral-sh/uv:${UV_VERSION} AS uv

FROM debian:trixie

ARG USERNAME=dev
ARG USER_UID=1000
ARG USER_GID=1000
ARG TYPST_VERSION=0.15.1
ARG DOTNET_CHANNEL=10.0

ENV LANG=C.UTF-8 \
    DEBIAN_FRONTEND=noninteractive \
    DOTNET_ROOT=/usr/share/dotnet \
    DOTNET_CLI_TELEMETRY_OPTOUT=1 \
    DOTNET_NOLOGO=1 \
    UV_LINK_MODE=copy \
    TYPST_FONT_PATHS=/workspaces/typst-pptx/fixtures/fonts \
    TYPST_IGNORE_SYSTEM_FONTS=true

# Base tools, LibreOffice (headless, no fonts on purpose: only fixture fonts are
# visible, see fonts.conf), xmllint, sshd, .NET runtime deps.
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      build-essential pkg-config curl ca-certificates git xz-utils unzip zip \
      make sudo less procps jq \
      openssh-server openssh-client \
      libxml2-utils fontconfig \
      python3 \
      libreoffice-nogui \
      libicu76 zlib1g \
 && rm -rf /var/lib/apt/lists/* \
 && mkdir -p /run/sshd

# Non-root user with passwordless sudo (the devcontainer CLI remaps the UID to
# the host user's on start).
RUN groupadd --gid ${USER_GID} ${USERNAME} \
 && useradd --uid ${USER_UID} --gid ${USER_GID} --create-home --shell /bin/bash ${USERNAME} \
 && echo "${USERNAME} ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/${USERNAME} \
 && chmod 0440 /etc/sudoers.d/${USERNAME}

# typst CLI (reference PDFs/PNGs).
RUN case "$(dpkg --print-architecture)" in \
      amd64) target=x86_64-unknown-linux-musl ;; \
      arm64) target=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported architecture" >&2; exit 1 ;; \
    esac \
 && curl -fsSL "https://github.com/typst/typst/releases/download/v${TYPST_VERSION}/typst-${target}.tar.xz" \
    | tar -xJ -C /tmp \
 && install -m 755 "/tmp/typst-${target}/typst" /usr/local/bin/typst \
 && rm -rf "/tmp/typst-${target}"

# .NET SDK for the Open XML SDK validator (tools/validate/oxml).
RUN curl -fsSL https://dot.net/v1/dotnet-install.sh -o /tmp/dotnet-install.sh \
 && bash /tmp/dotnet-install.sh --channel "${DOTNET_CHANNEL}" --install-dir "${DOTNET_ROOT}" \
 && ln -s "${DOTNET_ROOT}/dotnet" /usr/local/bin/dotnet \
 && rm /tmp/dotnet-install.sh

COPY --from=uv /uv /uvx /usr/local/bin/

# Fixture fonts only (bind-mounted repo path, so no rebuild when fonts change).
COPY .devcontainer/fonts.conf /etc/fonts/conf.d/50-typst-pptx-fixtures.conf
COPY .devcontainer/sshd_config /etc/typst-pptx/sshd_config

USER ${USERNAME}
ENV PATH=/home/${USERNAME}/.cargo/bin:${PATH}

# Mount points for the named volumes, created here so they start out owned by
# the user.
RUN mkdir -p ~/.cargo/registry ~/.cargo/git ~/.cache ~/.nuget

# Rust toolchain from rust-toolchain.toml, plus test tools.
COPY --chown=${USERNAME}:${USERNAME} rust-toolchain.toml /tmp/toolchain/rust-toolchain.toml
RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --no-modify-path --profile minimal --default-toolchain none \
 && cd /tmp/toolchain && rustup toolchain install \
 && rustup default "$(rustup show active-toolchain | cut -d' ' -f1)" \
 && curl -fsSL https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash \
 && cargo binstall -y --locked cargo-nextest@0.9 cargo-insta@1 \
 && rm -rf /tmp/toolchain ~/.cargo/registry/* ~/.cargo/git/*

WORKDIR /workspaces/typst-pptx
