# syntax=docker/dockerfile:1

FROM ghcr.io/yuki-js/dotfiles/dotimage:latest

USER root

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG OPENCODE_VERSION=2.0.16
ARG RUST_TOOLCHAIN=stable

ENV HOME=/root \
    RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH="/root/.cargo/bin:/root/.opencode/bin:${PATH}"

# System packages needed for SSH, OpenCode, and Rust development.
RUN apt-get update \
    && DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        libssl-dev \
        openssh-server \
        pkg-config \
        ssh-import-id \
    && rm -rf /var/lib/apt/lists/* \
    && install -d -m 0755 /run/sshd /home/user/codes/suicash \
    && chown user:user /home/user/codes/suicash

# Install the Rust toolchain through rustup.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain "$RUST_TOOLCHAIN" \
    && rustc --version \
    && cargo --version \
    && rustup show active-toolchain

# Install OpenCode CLI v2 for root.
RUN curl -fsSL https://opencode.ai/v2/install \
    | bash -s -- --version "$OPENCODE_VERSION" --no-modify-path \
    && install -d -m 0755 /usr/local/bin \
    && ln -sf /root/.opencode/bin/opencode /usr/local/bin/opencode \
    && opencode --version

# Import the current public SSH keys of the two trusted GitHub accounts.
# The image is a build-time snapshot; rebuild after key changes.
RUN install -d -m 0700 /root/.ssh \
    && rm -f /root/.ssh/authorized_keys \
    && ssh-import-id -o /root/.ssh/authorized_keys gh:yuki-js gh:serkenn \
    && grep -Fq 'ssh-import-id gh:yuki-js' /root/.ssh/authorized_keys \
    && grep -Fq 'ssh-import-id gh:serkenn' /root/.ssh/authorized_keys \
    && chmod 0700 /root/.ssh \
    && chmod 0600 /root/.ssh/authorized_keys

# SSH is key-only and accepts root only.
RUN printf '%s\n' \
    'Port 22' \
    'ListenAddress 0.0.0.0' \
    'PermitRootLogin prohibit-password' \
    'PubkeyAuthentication yes' \
    'PasswordAuthentication no' \
    'KbdInteractiveAuthentication no' \
    'ChallengeResponseAuthentication no' \
    'PermitEmptyPasswords no' \
    'AuthenticationMethods publickey' \
    'AuthorizedKeysFile /root/.ssh/authorized_keys' \
    'AllowUsers root' \
    'UsePAM yes' \
    'X11Forwarding no' \
    'PrintMotd no' \
    > /etc/ssh/sshd_config.suicash \
    && ssh-keygen -A \
    && /usr/sbin/sshd -t -f /etc/ssh/sshd_config.suicash

# Documentation only; publishing is handled by Kubernetes Service/Ingress.
EXPOSE 22/tcp
EXPOSE 1919/tcp
EXPOSE 4545/tcp
EXPOSE 14514/tcp

WORKDIR /home/user/codes/suicash

STOPSIGNAL SIGTERM

CMD ["/usr/sbin/sshd", "-D", "-e", "-f", "/etc/ssh/sshd_config.suicash"]
