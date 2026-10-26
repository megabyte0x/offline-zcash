# Runtime image for one mesh device. The binary is built on the host (Debian 12)
# and copied in by bench/mesh-bench.sh.
FROM debian:12-slim
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
      ca-certificates curl tcpdump iproute2 procps jq >/dev/null && rm -rf /var/lib/apt/lists/*
COPY ozm-node /usr/local/bin/ozm-node
ENTRYPOINT ["ozm-node"]
