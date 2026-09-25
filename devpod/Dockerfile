# syntax=docker/dockerfile:1

FROM ghcr.io/yuki-js/dotfiles/dotimage:latest

USER root

SHELL ["/bin/bash", "-o", "pipefail", "-c"]

ARG OPENCODE_VERSION=2.0.16
ARG RUST_TOOLCHAIN=stable

# Build-time HOME stays root; login sessions get HOME=/home/user from PAM.
ENV HOME=/root \
    RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH="/usr/local/cargo/bin:${PATH}"

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

# Install the Rust toolchain system-wide so that the unprivileged "user"
# account can use it. /root is unreadable once SSH stops accepting root.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --default-toolchain "$RUST_TOOLCHAIN" \
    && rustc --version \
    && cargo --version \
    && rustup show active-toolchain \
    && chmod -R a+rX "$RUSTUP_HOME" "$CARGO_HOME" \
    && install -d -m 0755 /usr/local/bin \
    && for b in "$CARGO_HOME"/bin/*; do \
        ln -sf "$b" "/usr/local/bin/$(basename "$b")"; \
    done

# Install OpenCode CLI v2 system-wide for the same reason.
RUN install -d -m 0755 /tmp/ocbuild \
    && HOME=/tmp/ocbuild curl -fsSL https://opencode.ai/v2/install \
    | HOME=/tmp/ocbuild bash -s -- --version "$OPENCODE_VERSION" --no-modify-path \
    && install -m 0755 /tmp/ocbuild/.opencode/bin/opencode /usr/local/bin/opencode \
    && rm -rf /tmp/ocbuild \
    && opencode --version

# Import the current public SSH keys of the two trusted GitHub accounts.
# They live outside /home/user on purpose: /home/user is a PVC mount, so
# anything baked into that path in the image would be hidden at runtime.
# The image is a build-time snapshot; rebuild after key changes.
RUN install -d -m 0755 /etc/ssh/authorized_keys.d \
    && rm -f /etc/ssh/authorized_keys.d/user \
    && ssh-import-id -o /etc/ssh/authorized_keys.d/user gh:yuki-js gh:serkenn \
    && grep -Fq 'ssh-import-id gh:yuki-js' /etc/ssh/authorized_keys.d/user \
    && grep -Fq 'ssh-import-id gh:serkenn' /etc/ssh/authorized_keys.d/user \
    && chown root:root /etc/ssh/authorized_keys.d/user \
    && chmod 0644 /etc/ssh/authorized_keys.d/user

# root must not be reachable over SSH at all.
RUN rm -rf /root/.ssh

# The unprivileged account escalates with sudo and no second factor.
RUN printf '%s\n' 'user ALL=(ALL) NOPASSWD: ALL' > /etc/sudoers.d/user \
    && chmod 0440 /etc/sudoers.d/user \
    && visudo -c -f /etc/sudoers.d/user

# Make the toolchain locations visible to login sessions, which do not
# inherit this image's ENV. PAM reads /etc/environment.
RUN printf '\n# START SUICASH SUDEVELOP CONFIG\nRUSTUP_HOME=/usr/local/rustup\nCARGO_HOME=/usr/local/cargo\n# STOP SUICASH SUDEVELOP CONFIG\n' >> /etc/environment

# SSH is key-only, accepts the "user" account only, and refuses root.
RUN printf '%s\n' \
    'Port 22' \
    'ListenAddress 0.0.0.0' \
    'PermitRootLogin no' \
    'PubkeyAuthentication yes' \
    'PasswordAuthentication no' \
    'KbdInteractiveAuthentication no' \
    'ChallengeResponseAuthentication no' \
    'PermitEmptyPasswords no' \
    'AuthenticationMethods publickey' \
    'AuthorizedKeysFile /etc/ssh/authorized_keys.d/%u' \
    'AllowUsers user' \
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

# Runs as root so sshd can bind port 22 and drop privileges per connection.
# Interactive logins are the unprivileged "user"; root is refused by sshd.
CMD ["/usr/sbin/sshd", "-D", "-e", "-f", "/etc/ssh/sshd_config.suicash"]
