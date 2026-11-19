# SPDX-License-Identifier: MIT OR Apache-2.0
# The image lpoco-bringup dt-check runs: the kernel's devicetree checks with their tools baked in, so nothing
# is installed at run time and a result cannot drift with the package index. Build, push, then name the pushed
# image by its repo digest: lpoco-bringup dt-check ... --image REGISTRY/NAME@sha256:DIGEST
#   docker build -t REGISTRY/lpoco-dt-check:YYYY-MM-DD -f bringup/dt-check/Dockerfile bringup/dt-check
# The base is alpine:edge (dtschema 2026.09 works on kernel v7.3; 3.24's 2025.12 crashes), pinned by digest.
FROM alpine@sha256:020dfcbaaf4cc1078bf2d9c7ba31a8466e334061dcd2f248001d68f79e52c000
RUN apk add --no-cache make gcc musl-dev flex bison yamllint py3-dt-schema dtc
