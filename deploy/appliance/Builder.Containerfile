FROM docker.io/oven/bun:1.4.3-debian@sha256:923bedb355f7746666659f71e724998159a5c8c54bd1cb1030e1975a0698415b
RUN apt-get update && apt-get install -y --no-install-recommends \
      libguestfs-tools qemu-utils unzip zip xfsprogs ca-certificates git \
      linux-image-$(dpkg --print-architecture) \
    && rm -rf /var/lib/apt/lists/*
ENV LIBGUESTFS_BACKEND=direct LIBGUESTFS_BACKEND_SETTINGS=force_tcg
WORKDIR /workspace
ENTRYPOINT ["bun", "tools/appliance/build.ts"]
